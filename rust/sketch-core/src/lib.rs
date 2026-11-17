mod color;
mod document;
mod error;
mod geometry;
mod outline;
mod session;

pub use color::Color;
pub use document::{parse_document, serialize_document, Document, Point, Stroke, FORMAT_VERSION};
pub use error::DocumentError;
pub use geometry::Affine;
pub use outline::{
    centre_line, outline_segments, outline_stroke, outline_tail, PathData, VERB_CLOSE, VERB_CUBIC,
    VERB_LINE, VERB_MOVE, VERB_QUAD,
};
pub use session::{Session, SessionState};
