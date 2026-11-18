//! The UniFFI surface used by the Swift and Kotlin views. Thin wrappers only.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::{
    outline_stroke, parse_document, serialize_document, Affine, Background, BackgroundError, Color,
    Document, DocumentError, InputSample, LiveUpdate, PathData, Pen, Session, SessionState,
};

#[derive(Debug, Clone, PartialEq, thiserror::Error, uniffi::Error)]
pub enum SketchError {
    #[error("{message}")]
    InvalidDocument { message: String },
    #[error("{message}")]
    InvalidBackground { message: String },
}

impl From<DocumentError> for SketchError {
    fn from(error: DocumentError) -> Self {
        SketchError::InvalidDocument {
            message: error.to_string(),
        }
    }
}

impl From<BackgroundError> for SketchError {
    fn from(error: BackgroundError) -> Self {
        SketchError::InvalidBackground {
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StrokeCommit {
    pub state: SessionState,
    pub path: PathData,
}

/// A committed stroke's outline with its fill colour.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommittedStroke {
    pub path: PathData,
    pub color: Color,
}

#[derive(uniffi::Object)]
pub struct SketchSession {
    session: Mutex<Session>,
}

impl SketchSession {
    fn new(document: Document) -> Arc<Self> {
        Arc::new(SketchSession {
            session: Mutex::new(Session::new(document)),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[uniffi::export]
impl SketchSession {
    #[uniffi::constructor]
    pub fn new_empty(width: f64, height: f64) -> Result<Arc<Self>, SketchError> {
        Ok(Self::new(Document::new(width, height)?))
    }

    #[uniffi::constructor]
    pub fn from_json(json: String) -> Result<Arc<Self>, SketchError> {
        Ok(Self::new(parse_document(&json)?))
    }

    pub fn to_json(&self) -> String {
        serialize_document(self.lock().document())
    }

    pub fn state(&self) -> SessionState {
        self.lock().state()
    }

    pub fn document_size(&self) -> Size {
        let session = self.lock();
        Size {
            width: session.document().width,
            height: session.document().height,
        }
    }

    pub fn undo(&self) -> SessionState {
        self.lock().undo()
    }

    pub fn redo(&self) -> SessionState {
        self.lock().redo()
    }

    pub fn clear(&self) -> SessionState {
        self.lock().clear()
    }

    /// Replaces the document and resets history; an invalid document leaves everything unchanged.
    pub fn load_json(&self, json: String) -> Result<SessionState, SketchError> {
        let document = parse_document(&json)?;
        Ok(self.lock().replace_document(document))
    }

    pub fn begin_stroke(&self, pen: Pen, view_to_doc: Affine, sample: InputSample) -> LiveUpdate {
        self.lock().begin_stroke(pen, view_to_doc, sample)
    }

    pub fn extend_stroke(
        &self,
        samples: Vec<InputSample>,
        predicted: Vec<InputSample>,
    ) -> LiveUpdate {
        self.lock().extend_stroke(&samples, &predicted)
    }

    pub fn end_stroke(&self) -> Option<StrokeCommit> {
        self.lock()
            .end_stroke()
            .map(|(state, path)| StrokeCommit { state, path })
    }

    pub fn cancel_stroke(&self) {
        self.lock().cancel_stroke();
    }

    pub fn has_live_stroke(&self) -> bool {
        self.lock().has_live_stroke()
    }

    pub fn committed_paths(&self) -> Vec<CommittedStroke> {
        self.lock()
            .document()
            .strokes
            .iter()
            .map(|stroke| CommittedStroke {
                path: outline_stroke(stroke),
                color: stroke.color,
            })
            .collect()
    }

    pub fn export_svg(&self, background: Option<String>) -> Result<String, SketchError> {
        let document = self.lock().document().clone();
        Ok(crate::export_svg(&document, background.as_deref())?)
    }
}

#[uniffi::export]
pub fn parse_background(svg: String) -> Result<Background, SketchError> {
    Ok(crate::parse_background(&svg)?)
}

#[uniffi::export]
pub fn fit_transform(
    content_width: f64,
    content_height: f64,
    box_width: f64,
    box_height: f64,
) -> Affine {
    crate::fit_transform(content_width, content_height, box_width, box_height)
}
