//! Flattens an SVG background into draw ops with `usvg`. Only `data:` image hrefs are read.

use std::collections::BTreeSet;
use std::sync::Mutex;

use usvg::roxmltree;
use usvg::tiny_skia_path::PathSegment;

use crate::draw_ops::{
    FillRule, GradientStop, ImageFormat, LineCap, LineJoin, Paint, Spread, StrokeParams,
};
use crate::{Affine, BackgroundError, Color, DrawOp, PathData};

/// Bounds the work and memory any background can cause, since `<use>` multiplies content.
const MAX_OPS: usize = 50_000;
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_PATH_COORDS: usize = 4_000_000;

#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BackgroundWarning {
    UnsupportedText,
    UnsupportedFilter,
    UnsupportedMask,
    UnsupportedPattern,
    UnsupportedExternalImage,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[derive(Debug, Clone, PartialEq)]
pub struct Background {
    pub width: f64,
    pub height: f64,
    pub ops: Vec<DrawOp>,
    pub warnings: Vec<BackgroundWarning>,
}

pub fn parse_background(svg: &str) -> Result<Background, BackgroundError> {
    let warnings = Mutex::new(BTreeSet::new());
    let default_data = usvg::ImageHrefResolver::default_data_resolver();
    let options = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|mime, data, options| {
                scan_for_text(&data, &warnings);
                default_data(mime, data, options)
            }),
            resolve_string: Box::new(|_, _| {
                record(&warnings, BackgroundWarning::UnsupportedExternalImage);
                None
            }),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| BackgroundError::Invalid {
        message: e.to_string(),
    })?;
    drop(options);
    scan_for_text(svg.as_bytes(), &warnings);

    let mut flattener = Flattener {
        ops: Vec::new(),
        budget: Budget::default(),
        over_limit: false,
        warnings: warnings.into_inner().unwrap_or_default(),
    };
    flattener.group(tree.root(), Affine::IDENTITY);
    if flattener.over_limit {
        return Err(BackgroundError::Invalid {
            message: format!(
                "background exceeds {MAX_OPS} draw ops or {MAX_IMAGE_BYTES} bytes of images"
            ),
        });
    }
    let size = tree.size();
    Ok(Background {
        width: f64::from(size.width()),
        height: f64::from(size.height()),
        ops: flattener.ops,
        warnings: flattener.warnings.into_iter().collect(),
    })
}

fn record(warnings: &Mutex<BTreeSet<BackgroundWarning>>, warning: BackgroundWarning) {
    if let Ok(mut set) = warnings.lock() {
        set.insert(warning);
    }
}

/// `usvg` is built without text support and drops `<text>` silently, so look for it in the XML.
fn scan_for_text(data: &[u8], warnings: &Mutex<BTreeSet<BackgroundWarning>>) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let Ok(document) = roxmltree::Document::parse_with_options(text, options) else {
        return;
    };
    if document.descendants().any(|node| node.has_tag_name("text")) {
        record(warnings, BackgroundWarning::UnsupportedText);
    }
}

#[derive(Default)]
struct Budget {
    ops: usize,
    path_coords: usize,
    image_bytes: usize,
}

struct Flattener {
    ops: Vec<DrawOp>,
    budget: Budget,
    over_limit: bool,
    warnings: BTreeSet<BackgroundWarning>,
}

impl Flattener {
    /// Counts work before it is allocated; false once any limit is passed.
    fn reserve(&mut self, ops: usize, path_coords: usize, image_bytes: usize) -> bool {
        self.budget.ops += ops;
        self.budget.path_coords += path_coords;
        self.budget.image_bytes += image_bytes;
        self.over_limit |= self.budget.ops > MAX_OPS
            || self.budget.path_coords > MAX_PATH_COORDS
            || self.budget.image_bytes > MAX_IMAGE_BYTES;
        !self.over_limit
    }

    fn emit(&mut self, op: DrawOp) {
        if self.reserve(1, 0, 0) {
            self.ops.push(op);
        }
    }

    fn group(&mut self, group: &usvg::Group, parent: Affine) {
        if self.over_limit {
            return;
        }
        let transform = affine(group.transform()).then(&parent);
        if !group.filters().is_empty() {
            self.warnings.insert(BackgroundWarning::UnsupportedFilter);
        }
        if group.mask().is_some() {
            self.warnings.insert(BackgroundWarning::UnsupportedMask);
        }
        let clips = group
            .clip_path()
            .map_or(0, |clip| self.push_clip(clip, transform));
        let layered = group.opacity().get() < 1.0;
        if layered {
            self.emit(DrawOp::PushLayer {
                opacity: group.opacity().get(),
            });
        }
        for child in group.children() {
            self.node(child, transform);
        }
        if layered {
            self.emit(DrawOp::PopLayer);
        }
        for _ in 0..clips {
            self.emit(DrawOp::PopClip);
        }
    }

    fn node(&mut self, node: &usvg::Node, transform: Affine) {
        match node {
            usvg::Node::Group(group) => self.group(group, transform),
            usvg::Node::Path(path) if path.is_visible() => self.path(path, transform),
            usvg::Node::Image(image) if image.is_visible() => self.image(image, transform),
            usvg::Node::Text(_) => {
                self.warnings.insert(BackgroundWarning::UnsupportedText);
            }
            usvg::Node::Path(_) | usvg::Node::Image(_) => {}
        }
    }

    fn path(&mut self, path: &usvg::Path, transform: Affine) {
        let painted = usize::from(path.fill().is_some()) + usize::from(path.stroke().is_some());
        if !self.reserve(painted, painted * path.data().points().len() * 2, 0) {
            return;
        }
        let data = path_data(path.data());
        let fill = path.fill().and_then(|fill| {
            let paint = self.paint(fill.paint())?;
            Some(DrawOp::FillPath {
                path: data.clone(),
                paint,
                rule: fill_rule(fill.rule()),
                opacity: fill.opacity().get(),
                transform,
            })
        });
        let stroke = path.stroke().and_then(|stroke| {
            let paint = self.paint(stroke.paint())?;
            Some(DrawOp::StrokePath {
                path: data.clone(),
                paint,
                params: StrokeParams {
                    width: stroke.width().get(),
                    cap: match stroke.linecap() {
                        usvg::LineCap::Butt => LineCap::Butt,
                        usvg::LineCap::Round => LineCap::Round,
                        usvg::LineCap::Square => LineCap::Square,
                    },
                    join: match stroke.linejoin() {
                        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoin::Miter,
                        usvg::LineJoin::Round => LineJoin::Round,
                        usvg::LineJoin::Bevel => LineJoin::Bevel,
                    },
                    miter_limit: stroke.miterlimit().get(),
                    dash: stroke.dasharray().map(<[f32]>::to_vec).unwrap_or_default(),
                    dash_offset: stroke.dashoffset(),
                },
                opacity: stroke.opacity().get(),
                transform,
            })
        });
        let ordered = match path.paint_order() {
            usvg::PaintOrder::FillAndStroke => [fill, stroke],
            usvg::PaintOrder::StrokeAndFill => [stroke, fill],
        };
        self.ops.extend(ordered.into_iter().flatten());
    }

    fn image(&mut self, image: &usvg::Image, transform: Affine) {
        let (data, format) = match image.kind() {
            usvg::ImageKind::SVG(tree) => return self.group(tree.root(), transform),
            usvg::ImageKind::PNG(data) => (data, ImageFormat::Png),
            usvg::ImageKind::JPEG(data) => (data, ImageFormat::Jpeg),
            usvg::ImageKind::GIF(data) => (data, ImageFormat::Gif),
            usvg::ImageKind::WEBP(data) => (data, ImageFormat::Webp),
        };
        if !self.reserve(1, 0, data.len()) {
            return;
        }
        let size = image.size();
        self.ops.push(DrawOp::Image {
            data: data.to_vec(),
            format,
            x: 0.0,
            y: 0.0,
            width: size.width(),
            height: size.height(),
            transform,
        });
    }

    /// Pushes the clip and any clip it is itself clipped by; returns how many pushes to pop.
    fn push_clip(&mut self, clip: &usvg::ClipPath, transform: Affine) -> usize {
        if self.over_limit {
            return 0;
        }
        let mut shapes = Vec::new();
        collect_clip_shapes(clip.root(), Affine::IDENTITY, &mut shapes);
        let rule = match shapes.as_slice() {
            [(_, rule)] => *rule,
            _ => FillRule::NonZero,
        };
        let mut path = PathData::default();
        for (shape, _) in &shapes {
            path.append(shape);
        }
        if !self.reserve(1, path.coords.len(), 0) {
            return 0;
        }
        self.ops.push(DrawOp::PushClip {
            path,
            rule,
            transform: affine(clip.transform()).then(&transform),
        });
        1 + clip
            .clip_path()
            .map_or(0, |inner| self.push_clip(inner, transform))
    }

    fn paint(&mut self, paint: &usvg::Paint) -> Option<Paint> {
        match paint {
            usvg::Paint::Color(c) => Some(Paint::Solid(Color {
                r: c.red,
                g: c.green,
                b: c.blue,
                a: u8::MAX,
            })),
            usvg::Paint::LinearGradient(g) => Some(Paint::LinearGradient {
                x1: g.x1(),
                y1: g.y1(),
                x2: g.x2(),
                y2: g.y2(),
                stops: stops(g.stops()),
                spread: spread(g.spread_method()),
                transform: affine(g.transform()),
            }),
            usvg::Paint::RadialGradient(g) => Some(Paint::RadialGradient {
                cx: g.cx(),
                cy: g.cy(),
                r: g.r().get(),
                fx: g.fx(),
                fy: g.fy(),
                stops: stops(g.stops()),
                spread: spread(g.spread_method()),
                transform: affine(g.transform()),
            }),
            usvg::Paint::Pattern(_) => {
                self.warnings.insert(BackgroundWarning::UnsupportedPattern);
                None
            }
        }
    }
}

/// Clip shapes with their transforms baked into the coordinates, relative to the clip's space.
fn collect_clip_shapes(
    group: &usvg::Group,
    parent: Affine,
    shapes: &mut Vec<(PathData, FillRule)>,
) {
    let transform = affine(group.transform()).then(&parent);
    for child in group.children() {
        match child {
            usvg::Node::Group(inner) => collect_clip_shapes(inner, transform, shapes),
            usvg::Node::Path(path) if path.is_visible() => {
                let rule = path
                    .fill()
                    .map_or(FillRule::NonZero, |fill| fill_rule(fill.rule()));
                shapes.push((transformed(&path_data(path.data()), &transform), rule));
            }
            _ => {}
        }
    }
}

fn transformed(path: &PathData, transform: &Affine) -> PathData {
    let mut coords = Vec::with_capacity(path.coords.len());
    for xy in path.coords.chunks(2) {
        let (x, y) = transform.apply(f64::from(xy[0]), f64::from(xy[1]));
        coords.extend([x as f32, y as f32]);
    }
    PathData {
        verbs: path.verbs.clone(),
        coords,
    }
}

fn path_data(path: &usvg::tiny_skia_path::Path) -> PathData {
    let mut data = PathData::default();
    let f = f64::from;
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => data.move_to(f(p.x), f(p.y)),
            PathSegment::LineTo(p) => data.line_to(f(p.x), f(p.y)),
            PathSegment::QuadTo(c, p) => data.quad_to(f(c.x), f(c.y), f(p.x), f(p.y)),
            PathSegment::CubicTo(c1, c2, p) => {
                data.cubic_to(f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p.x), f(p.y))
            }
            PathSegment::Close => data.close(),
        }
    }
    data
}

fn affine(t: usvg::Transform) -> Affine {
    Affine {
        a: f64::from(t.sx),
        b: f64::from(t.ky),
        c: f64::from(t.kx),
        d: f64::from(t.sy),
        e: f64::from(t.tx),
        f: f64::from(t.ty),
    }
}

fn fill_rule(rule: usvg::FillRule) -> FillRule {
    match rule {
        usvg::FillRule::NonZero => FillRule::NonZero,
        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
    }
}

fn spread(method: usvg::SpreadMethod) -> Spread {
    match method {
        usvg::SpreadMethod::Pad => Spread::Pad,
        usvg::SpreadMethod::Reflect => Spread::Reflect,
        usvg::SpreadMethod::Repeat => Spread::Repeat,
    }
}

fn stops(stops: &[usvg::Stop]) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|stop| {
            let c = stop.color();
            GradientStop {
                offset: stop.offset().get(),
                color: Color {
                    r: c.red,
                    g: c.green,
                    b: c.blue,
                    a: (stop.opacity().get() * 255.0).round() as u8,
                },
            }
        })
        .collect()
}
