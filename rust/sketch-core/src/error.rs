use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DocumentError {
    #[error("malformed document: {message}")]
    Malformed { message: String },
    #[error("unsupported document version {found}")]
    UnsupportedVersion { found: u64 },
    #[error("document width and height must be finite, positive and at most 1,000,000")]
    InvalidSize,
    #[error("stroke {stroke} has an invalid color")]
    InvalidColor { stroke: usize },
    #[error("stroke {stroke} has no points")]
    EmptyStroke { stroke: usize },
    #[error("stroke {stroke} point {point} has an invalid coordinate or width")]
    InvalidWidth { stroke: usize, point: usize },
    #[error("document exceeds {limit} points or centre line samples")]
    TooLarge { limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BackgroundError {
    #[error("invalid background svg: {message}")]
    Invalid { message: String },
}
