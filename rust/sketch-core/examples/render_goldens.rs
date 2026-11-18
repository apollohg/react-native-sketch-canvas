//! Renders each `fixtures/goldens/<scene>.json` to `<scene>.png` with tiny-skia, interpreting the
//! crate's own draw ops. The native renderers are tested against these images.

use std::path::Path;

use sketch_core::{
    fit_transform, outline_stroke, parse_background, parse_document, Affine, Color, DrawOp,
    FillRule, GradientStop, ImageFormat, LineCap, LineJoin, Paint, PathData, Spread, VERB_CLOSE,
    VERB_CUBIC, VERB_LINE, VERB_MOVE, VERB_QUAD,
};
use tiny_skia::{Mask, Pixmap, PixmapPaint, Transform};

fn main() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let goldens = fixtures.join("goldens");
    let mut scenes: Vec<_> = std::fs::read_dir(&goldens)
        .expect("goldens dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    scenes.sort();
    for scene_path in scenes {
        let scene: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&scene_path).expect("read scene"))
                .expect("scene json");
        let width = scene["width"].as_u64().expect("width") as u32;
        let height = scene["height"].as_u64().expect("height") as u32;
        let document = parse_document(&scene["document"].to_string()).expect("document");
        let mut renderer = Renderer::new(width, height);
        if let Some(hex) = scene["backgroundColor"].as_str() {
            let c = Color::parse(hex).expect("backgroundColor");
            renderer
                .target()
                .fill(tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a));
        }
        let doc_to_view = fit_transform(
            document.width,
            document.height,
            f64::from(width),
            f64::from(height),
        );
        if let Some(name) = scene["background"].as_str() {
            let svg = std::fs::read_to_string(fixtures.join("backgrounds").join(name))
                .expect("background");
            let background = parse_background(&svg).expect("background parses");
            let bg_to_doc = fit_transform(
                background.width,
                background.height,
                document.width,
                document.height,
            );
            renderer.draw(&background.ops, &bg_to_doc.then(&doc_to_view));
        }
        for stroke in &document.strokes {
            renderer.fill(
                &outline_stroke(stroke),
                &Paint::Solid(stroke.color),
                FillRule::NonZero,
                1.0,
                &doc_to_view,
            );
        }
        let output = scene_path.with_extension("png");
        renderer
            .layers
            .remove(0)
            .save_png(&output)
            .expect("write png");
        println!("wrote {}", output.display());
    }
}

struct Renderer {
    layers: Vec<Pixmap>,
    layer_opacity: Vec<f32>,
    masks: Vec<Option<Mask>>,
}

impl Renderer {
    fn new(width: u32, height: u32) -> Renderer {
        Renderer {
            layers: vec![Pixmap::new(width, height).expect("pixmap")],
            layer_opacity: Vec::new(),
            masks: vec![None],
        }
    }

    fn target(&mut self) -> &mut Pixmap {
        self.layers.last_mut().expect("layer")
    }

    fn mask(&self) -> Option<&Mask> {
        self.masks.last().and_then(Option::as_ref)
    }

    fn draw(&mut self, ops: &[DrawOp], base: &Affine) {
        for op in ops {
            match op {
                DrawOp::FillPath {
                    path,
                    paint,
                    rule,
                    opacity,
                    transform,
                } => {
                    self.fill(path, paint, *rule, *opacity, &transform.then(base));
                }
                DrawOp::StrokePath {
                    path,
                    paint,
                    params,
                    opacity,
                    transform,
                } => {
                    let Some(skia_path) = skia_path(path) else {
                        continue;
                    };
                    let mut stroke = tiny_skia::Stroke {
                        width: params.width,
                        miter_limit: params.miter_limit,
                        line_cap: match params.cap {
                            LineCap::Butt => tiny_skia::LineCap::Butt,
                            LineCap::Round => tiny_skia::LineCap::Round,
                            LineCap::Square => tiny_skia::LineCap::Square,
                        },
                        line_join: match params.join {
                            LineJoin::Miter => tiny_skia::LineJoin::Miter,
                            LineJoin::Round => tiny_skia::LineJoin::Round,
                            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
                        },
                        dash: None,
                    };
                    if !params.dash.is_empty() {
                        stroke.dash =
                            tiny_skia::StrokeDash::new(params.dash.clone(), params.dash_offset);
                    }
                    let skia_paint = skia_paint(paint, *opacity);
                    let ts = skia_transform(&transform.then(base));
                    let mask = self.mask().cloned();
                    self.target()
                        .stroke_path(&skia_path, &skia_paint, &stroke, ts, mask.as_ref());
                }
                DrawOp::PushClip {
                    path,
                    rule,
                    transform,
                } => {
                    let size = (self.target().width(), self.target().height());
                    let mut mask = self.mask().cloned().unwrap_or_else(|| {
                        let mut full = Mask::new(size.0, size.1).expect("mask");
                        full.data_mut().fill(u8::MAX);
                        full
                    });
                    if let Some(skia_path) = skia_path(path) {
                        mask.intersect_path(
                            &skia_path,
                            fill_rule(*rule),
                            true,
                            skia_transform(&transform.then(base)),
                        );
                    } else {
                        mask.data_mut().fill(0);
                    }
                    self.masks.push(Some(mask));
                }
                DrawOp::PopClip => {
                    self.masks.pop();
                }
                DrawOp::PushLayer { opacity } => {
                    let (w, h) = (self.target().width(), self.target().height());
                    self.layers.push(Pixmap::new(w, h).expect("layer"));
                    self.layer_opacity.push(*opacity);
                }
                DrawOp::PopLayer => {
                    let layer = self.layers.pop().expect("layer");
                    let opacity = self.layer_opacity.pop().expect("layer opacity");
                    let mask = self.mask().cloned();
                    let paint = PixmapPaint {
                        opacity,
                        ..PixmapPaint::default()
                    };
                    self.target().draw_pixmap(
                        0,
                        0,
                        layer.as_ref(),
                        &paint,
                        Transform::identity(),
                        mask.as_ref(),
                    );
                }
                DrawOp::Image {
                    data,
                    format,
                    x,
                    y,
                    width,
                    height,
                    transform,
                } => {
                    assert_eq!(
                        *format,
                        ImageFormat::Png,
                        "golden scenes only use PNG images"
                    );
                    let image = Pixmap::decode_png(data).expect("png");
                    let place = Affine {
                        a: f64::from(*width) / f64::from(image.width()),
                        b: 0.0,
                        c: 0.0,
                        d: f64::from(*height) / f64::from(image.height()),
                        e: f64::from(*x),
                        f: f64::from(*y),
                    };
                    let paint = PixmapPaint {
                        quality: tiny_skia::FilterQuality::Bilinear,
                        ..PixmapPaint::default()
                    };
                    let ts = skia_transform(&place.then(transform).then(base));
                    let mask = self.mask().cloned();
                    self.target()
                        .draw_pixmap(0, 0, image.as_ref(), &paint, ts, mask.as_ref());
                }
            }
        }
    }

    fn fill(
        &mut self,
        path: &PathData,
        paint: &Paint,
        rule: FillRule,
        opacity: f32,
        transform: &Affine,
    ) {
        let Some(skia_path) = skia_path(path) else {
            return;
        };
        let skia_paint = skia_paint(paint, opacity);
        let mask = self.mask().cloned();
        self.target().fill_path(
            &skia_path,
            &skia_paint,
            fill_rule(rule),
            skia_transform(transform),
            mask.as_ref(),
        );
    }
}

fn skia_path(data: &PathData) -> Option<tiny_skia::Path> {
    let mut builder = tiny_skia::PathBuilder::new();
    let mut c = data.coords.iter().copied();
    let mut next = || c.next().expect("coords");
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

fn skia_transform(t: &Affine) -> Transform {
    Transform::from_row(
        t.a as f32, t.b as f32, t.c as f32, t.d as f32, t.e as f32, t.f as f32,
    )
}

fn fill_rule(rule: FillRule) -> tiny_skia::FillRule {
    match rule {
        FillRule::NonZero => tiny_skia::FillRule::Winding,
        FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    }
}

fn skia_color(c: Color, opacity: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, (f32::from(c.a) * opacity).round() as u8)
}

fn skia_stops(stops: &[GradientStop], opacity: f32) -> Vec<tiny_skia::GradientStop> {
    stops
        .iter()
        .map(|s| tiny_skia::GradientStop::new(s.offset, skia_color(s.color, opacity)))
        .collect()
}

fn spread(spread: Spread) -> tiny_skia::SpreadMode {
    match spread {
        Spread::Pad => tiny_skia::SpreadMode::Pad,
        Spread::Reflect => tiny_skia::SpreadMode::Reflect,
        Spread::Repeat => tiny_skia::SpreadMode::Repeat,
    }
}

fn skia_paint(paint: &Paint, opacity: f32) -> tiny_skia::Paint<'static> {
    let mut skia = tiny_skia::Paint {
        anti_alias: true,
        ..tiny_skia::Paint::default()
    };
    match paint {
        Paint::Solid(c) => skia.set_color(skia_color(*c, opacity)),
        Paint::LinearGradient {
            x1,
            y1,
            x2,
            y2,
            stops,
            spread: s,
            transform,
        } => {
            skia.shader = tiny_skia::LinearGradient::new(
                tiny_skia::Point::from_xy(*x1, *y1),
                tiny_skia::Point::from_xy(*x2, *y2),
                skia_stops(stops, opacity),
                spread(*s),
                skia_transform(transform),
            )
            .expect("linear gradient");
        }
        Paint::RadialGradient {
            cx,
            cy,
            r,
            fx,
            fy,
            stops,
            spread: s,
            transform,
        } => {
            skia.shader = tiny_skia::RadialGradient::new(
                tiny_skia::Point::from_xy(*fx, *fy),
                tiny_skia::Point::from_xy(*cx, *cy),
                *r,
                skia_stops(stops, opacity),
                spread(*s),
                skia_transform(transform),
            )
            .expect("radial gradient");
        }
    }
    skia
}
