use crate::{Affine, Color, PathData};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spread {
    Pad,
    Reflect,
    Repeat,
}

/// Gradient coordinates are in the op's local space, mapped by `transform` first.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Solid(Color),
    LinearGradient {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: Vec<GradientStop>,
        spread: Spread,
        transform: Affine,
    },
    RadialGradient {
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: Vec<GradientStop>,
        spread: Spread,
        transform: Affine,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeParams {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
    pub dash: Vec<f32>,
    pub dash_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

/// One drawing instruction. `transform` maps the op's coordinates into the background's space.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    FillPath {
        path: PathData,
        paint: Paint,
        rule: FillRule,
        opacity: f32,
        transform: Affine,
    },
    StrokePath {
        path: PathData,
        paint: Paint,
        params: StrokeParams,
        opacity: f32,
        transform: Affine,
    },
    PushClip {
        path: PathData,
        rule: FillRule,
        transform: Affine,
    },
    PopClip,
    PushLayer {
        opacity: f32,
    },
    PopLayer,
    /// Encoded image bytes drawn into the rect `x, y, width, height`.
    Image {
        data: Vec<u8>,
        format: ImageFormat,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        transform: Affine,
    },
}
