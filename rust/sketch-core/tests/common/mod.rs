#![allow(dead_code)]

use sketch_core::{
    parse_document, Document, PathData, VERB_CLOSE, VERB_CUBIC, VERB_LINE, VERB_MOVE, VERB_QUAD,
};
use std::path::{Path, PathBuf};

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

pub fn load_fixture_document(relative: &str) -> Document {
    let path = fixtures_dir().join(relative);
    let json =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    parse_document(&json).unwrap_or_else(|e| panic!("parse {}: {e:?}", path.display()))
}

pub fn to_skia_path(data: &PathData) -> Option<tiny_skia::Path> {
    let mut builder = tiny_skia::PathBuilder::new();
    let mut c = data.coords.iter().copied();
    let mut next = || c.next().expect("coords exhausted before verbs");
    for &verb in &data.verbs {
        match verb {
            VERB_MOVE => builder.move_to(next(), next()),
            VERB_LINE => builder.line_to(next(), next()),
            VERB_QUAD => builder.quad_to(next(), next(), next(), next()),
            VERB_CUBIC => builder.cubic_to(next(), next(), next(), next(), next(), next()),
            VERB_CLOSE => builder.close(),
            other => panic!("unknown verb {other}"),
        }
    }
    builder.finish()
}

/// Coverage raster: pixel (px, py) covers document area [px - offset_x, …+1) × [py - offset_y, …+1).
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub alpha: Vec<u8>,
}

impl Raster {
    pub fn coverage_at_doc(&self, x: f32, y: f32) -> f32 {
        let px = (x + self.offset_x).floor() as i64;
        let py = (y + self.offset_y).floor() as i64;
        if px < 0 || py < 0 || px >= self.width as i64 || py >= self.height as i64 {
            return 0.0;
        }
        f32::from(self.alpha[(py as u32 * self.width + px as u32) as usize]) / 255.0
    }

    pub fn total_coverage(&self) -> f64 {
        self.alpha.iter().map(|&a| f64::from(a) / 255.0).sum()
    }

    /// Document-space centre of every pixel, with its coverage.
    pub fn pixels(&self) -> impl Iterator<Item = (f32, f32, f32)> + '_ {
        (0..self.height).flat_map(move |py| {
            (0..self.width).map(move |px| {
                let a = f32::from(self.alpha[(py * self.width + px) as usize]) / 255.0;
                (
                    px as f32 + 0.5 - self.offset_x,
                    py as f32 + 0.5 - self.offset_y,
                    a,
                )
            })
        })
    }
}

pub fn rasterize(data: &PathData, margin: f32) -> Raster {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for xy in data.coords.chunks(2) {
        min_x = min_x.min(xy[0]);
        max_x = max_x.max(xy[0]);
        min_y = min_y.min(xy[1]);
        max_y = max_y.max(xy[1]);
    }
    let offset_x = (margin - min_x).ceil();
    let offset_y = (margin - min_y).ceil();
    let width = (max_x + offset_x + margin).ceil() as u32;
    let height = (max_y + offset_y + margin).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("pixmap size");
    let path = to_skia_path(data).expect("non-empty path");
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::from_translate(offset_x, offset_y),
        None,
    );
    let alpha = pixmap.pixels().iter().map(|p| p.alpha()).collect();
    Raster {
        width,
        height,
        offset_x,
        offset_y,
        alpha,
    }
}

/// Splits a path into subpaths, each a list of (verb, coords).
pub fn subpaths(data: &PathData) -> Vec<Vec<(u8, Vec<f32>)>> {
    let mut out: Vec<Vec<(u8, Vec<f32>)>> = Vec::new();
    let mut i = 0;
    for &verb in &data.verbs {
        let n = match verb {
            VERB_MOVE | VERB_LINE => 2,
            VERB_QUAD => 4,
            VERB_CUBIC => 6,
            _ => 0,
        };
        if verb == VERB_MOVE {
            out.push(Vec::new());
        }
        out.last_mut()
            .expect("path starts with move")
            .push((verb, data.coords[i..i + n].to_vec()));
        i += n;
    }
    out
}

/// Shoelace area over every on-curve and control point of a subpath, in order.
pub fn shoelace_area(subpath: &[(u8, Vec<f32>)]) -> f64 {
    let pts: Vec<(f64, f64)> = subpath
        .iter()
        .flat_map(|(_, c)| {
            c.chunks(2)
                .map(|p| (f64::from(p[0]), f64::from(p[1])))
                .collect::<Vec<_>>()
        })
        .collect();
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1)
        .sum::<f64>()
        / 2.0
}
