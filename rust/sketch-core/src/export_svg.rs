use std::fmt::Write;

use usvg::roxmltree;

use crate::document::format_number;
use crate::{
    outline_stroke, parse_background, BackgroundError, Color, Document, PathData, VERB_CLOSE,
    VERB_CUBIC, VERB_LINE, VERB_MOVE, VERB_QUAD,
};

/// Root attributes replaced when nesting a background, so the nested `<svg>` fills the document
/// and its default `preserveAspectRatio` performs the aspect-fit.
const REPLACED_BACKGROUND_ATTRIBUTES: [&str; 5] =
    ["x", "y", "width", "height", "preserveAspectRatio"];

/// A standalone SVG of the document: strokes as filled outlines, over the optional background.
pub fn export_svg(doc: &Document, background: Option<&str>) -> Result<String, BackgroundError> {
    let (width, height) = (format_number(doc.width), format_number(doc.height));
    let nested = background
        .map(|svg| nested_background(svg, &width, &height))
        .transpose()?;
    let mut out = String::new();
    if let Some(subset) = nested.as_ref().and_then(|n| n.dtd_subset) {
        let _ = write!(out, "<!DOCTYPE svg [{subset}]>");
    }
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    );
    if let Some(nested) = nested {
        out.push_str(&nested.element);
    }
    for stroke in &doc.strokes {
        let color = stroke.color;
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}""#,
            path_d(&outline_stroke(stroke)),
            Color {
                a: u8::MAX,
                ..color
            }
            .to_hex()
        );
        if color.a < u8::MAX {
            let _ = write!(
                out,
                r#" fill-opacity="{}""#,
                format_number(f64::from(color.a) / f64::from(u8::MAX))
            );
        }
        out.push_str("/>");
    }
    out.push_str("</svg>");
    Ok(out)
}

struct NestedBackground<'a> {
    /// Entity declarations the element may reference, from the DOCTYPE's `[…]` subset.
    dtd_subset: Option<&'a str>,
    element: String,
}

fn internal_dtd_subset(prolog: &str) -> Option<&str> {
    let doctype = &prolog[prolog.find("<!DOCTYPE")?..];
    let open = doctype.find('[')?;
    let close = doctype.rfind(']').filter(|&close| close > open)?;
    Some(&doctype[open + 1..close])
}

/// The background's root element only, with its position and size attributes rewritten.
fn nested_background<'a>(
    svg: &'a str,
    width: &str,
    height: &str,
) -> Result<NestedBackground<'a>, BackgroundError> {
    let invalid = |message: String| BackgroundError::Invalid { message };
    let background = parse_background(svg)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let xml = roxmltree::Document::parse_with_options(svg, options)
        .map_err(|e| invalid(e.to_string()))?;
    let root = xml.root_element();
    let element = root.range();
    let name_end = svg[element.start + 1..element.end]
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .map_or(element.end, |offset| element.start + 1 + offset);

    let mut nested = String::with_capacity(element.len() + 96);
    nested.push_str(&svg[element.start..name_end]);
    let _ = write!(nested, r#" x="0" y="0" width="{width}" height="{height}""#);
    if root.attribute("viewBox").is_none() {
        let _ = write!(
            nested,
            r#" viewBox="0 0 {} {}""#,
            format_number(background.width),
            format_number(background.height)
        );
    }
    let mut cursor = name_end;
    for attribute in root.attributes() {
        if attribute.namespace().is_none()
            && REPLACED_BACKGROUND_ATTRIBUTES.contains(&attribute.name())
        {
            let range = attribute.range();
            nested.push_str(&svg[cursor..range.start]);
            cursor = range.end;
        }
    }
    nested.push_str(&svg[cursor..element.end]);
    Ok(NestedBackground {
        dtd_subset: internal_dtd_subset(&svg[..element.start]),
        element: nested,
    })
}

fn path_d(path: &PathData) -> String {
    let mut d = String::with_capacity(path.coords.len() * 7);
    let mut coords = path.coords.iter().map(|&c| format_number(f64::from(c)));
    for &verb in &path.verbs {
        let (command, count) = match verb {
            VERB_MOVE => ('M', 2),
            VERB_LINE => ('L', 2),
            VERB_QUAD => ('Q', 4),
            VERB_CUBIC => ('C', 6),
            VERB_CLOSE => ('Z', 0),
            _ => continue,
        };
        d.push(command);
        for i in 0..count {
            if i > 0 {
                d.push(' ');
            }
            d.push_str(&coords.next().unwrap_or_default());
        }
    }
    d
}
