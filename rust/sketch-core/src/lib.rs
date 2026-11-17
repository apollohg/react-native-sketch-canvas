mod color;
mod document;
mod error;

pub use color::Color;
pub use document::{parse_document, serialize_document, Document, Point, Stroke, FORMAT_VERSION};
pub use error::DocumentError;
