mod background;
mod capture;
mod color;
mod document;
mod draw_ops;
mod error;
mod export_svg;
mod fit;
mod geometry;
mod outline;
mod session;

pub use background::{parse_background, Background, BackgroundWarning};
pub use capture::{InputSample, LiveUpdate, Pen, PointerKind, TAIL_MAX_VERBS};
pub use color::Color;
pub use document::{parse_document, serialize_document, Document, Point, Stroke, FORMAT_VERSION};
pub use draw_ops::{
    DrawOp, FillRule, GradientStop, ImageFormat, LineCap, LineJoin, Paint, Spread, StrokeParams,
};
pub use error::{BackgroundError, DocumentError};
pub use export_svg::export_svg;
pub use fit::fit_transform;
pub use geometry::Affine;
pub use outline::{
    centre_line, outline_segments, outline_stroke, outline_tail, PathData, VERB_CLOSE, VERB_CUBIC,
    VERB_LINE, VERB_MOVE, VERB_QUAD,
};
pub use session::{Session, SessionState};
