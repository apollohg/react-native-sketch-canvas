use serde::Deserialize;
use serde_json::Value;

use crate::{Color, DocumentError};

pub const FORMAT_VERSION: u64 = 1;

const STORED_DECIMALS_SCALE: f64 = 100.0;
const MIN_STORED_WIDTH: f64 = 1.0 / STORED_DECIMALS_SCALE;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub width: f64,
    pub height: f64,
    pub strokes: Vec<Stroke>,
}

impl Document {
    pub fn new(width: f64, height: f64) -> Result<Document, DocumentError> {
        if !is_valid_size(width) || !is_valid_size(height) {
            return Err(DocumentError::InvalidSize);
        }
        Ok(Document {
            width,
            height,
            strokes: Vec::new(),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDocument {
    width: f64,
    height: f64,
    strokes: Vec<WireStroke>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStroke {
    color: String,
    points: Vec<[f64; 3]>,
}

fn malformed(error: impl ToString) -> DocumentError {
    DocumentError::Malformed {
        message: error.to_string(),
    }
}

fn is_valid_size(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

pub fn parse_document(json: &str) -> Result<Document, DocumentError> {
    let mut value: Value = serde_json::from_str(json).map_err(malformed)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| malformed("document must be an object"))?;
    let version = fields
        .remove("version")
        .ok_or_else(|| malformed("missing field `version`"))?
        .as_u64()
        .ok_or_else(|| malformed("`version` must be a positive integer"))?;
    if version != FORMAT_VERSION {
        return Err(DocumentError::UnsupportedVersion { found: version });
    }
    let wire: WireDocument = serde_json::from_value(value).map_err(malformed)?;
    let mut document = Document::new(wire.width, wire.height)?;
    for (stroke_index, wire_stroke) in wire.strokes.into_iter().enumerate() {
        let color = Color::parse(&wire_stroke.color).ok_or(DocumentError::InvalidColor {
            stroke: stroke_index,
        })?;
        if wire_stroke.points.is_empty() {
            return Err(DocumentError::EmptyStroke {
                stroke: stroke_index,
            });
        }
        let points = wire_stroke
            .points
            .into_iter()
            .enumerate()
            .map(|(point_index, [x, y, w])| {
                if x.is_finite() && y.is_finite() && w.is_finite() && w > 0.0 {
                    Ok(Point { x, y, w })
                } else {
                    Err(DocumentError::InvalidWidth {
                        stroke: stroke_index,
                        point: point_index,
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        document.strokes.push(Stroke { color, points });
    }
    Ok(document)
}

pub(crate) fn format_number(value: f64) -> String {
    let rounded = (value * STORED_DECIMALS_SCALE).round() / STORED_DECIMALS_SCALE;
    if rounded == 0.0 {
        return "0".to_string();
    }
    let text = format!("{rounded:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn serialize_document(doc: &Document) -> String {
    let mut out = format!(
        r#"{{"version":{FORMAT_VERSION},"width":{},"height":{},"strokes":["#,
        format_number(doc.width),
        format_number(doc.height)
    );
    for (stroke_index, stroke) in doc.strokes.iter().enumerate() {
        if stroke_index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"color":"{}","points":["#,
            stroke.color.to_hex()
        ));
        for (point_index, point) in stroke.points.iter().enumerate() {
            if point_index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "[{},{},{}]",
                format_number(point.x),
                format_number(point.y),
                format_number(point.w.max(MIN_STORED_WIDTH))
            ));
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}
