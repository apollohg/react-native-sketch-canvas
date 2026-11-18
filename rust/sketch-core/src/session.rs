use crate::capture::StrokeBuilder;
use crate::{outline_stroke, Affine, Document, InputSample, LiveUpdate, PathData, Pen, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionState {
    pub stroke_count: u32,
    pub can_undo: bool,
    pub can_redo: bool,
    pub revision: u64,
}

#[derive(Debug, Clone)]
enum Edit {
    AddStroke(Stroke),
    Clear(Vec<Stroke>),
}

#[derive(Debug)]
pub struct Session {
    document: Document,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
    revision: u64,
    live: Option<StrokeBuilder>,
}

impl Session {
    pub fn new(document: Document) -> Session {
        Session {
            document,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
            live: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn state(&self) -> SessionState {
        SessionState {
            stroke_count: u32::try_from(self.document.strokes.len()).unwrap_or(u32::MAX),
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            revision: self.revision,
        }
    }

    pub fn add_stroke(&mut self, stroke: Stroke) -> SessionState {
        self.document.strokes.push(stroke.clone());
        self.record(Edit::AddStroke(stroke))
    }

    pub fn clear(&mut self) -> SessionState {
        self.cancel_stroke();
        if self.document.strokes.is_empty() {
            return self.state();
        }
        let removed = std::mem::take(&mut self.document.strokes);
        self.record(Edit::Clear(removed))
    }

    pub fn undo(&mut self) -> SessionState {
        self.cancel_stroke();
        let Some(edit) = self.undo_stack.pop() else {
            return self.state();
        };
        match &edit {
            Edit::AddStroke(_) => {
                self.document.strokes.pop();
            }
            Edit::Clear(removed) => self.document.strokes = removed.clone(),
        }
        self.redo_stack.push(edit);
        self.bump()
    }

    pub fn redo(&mut self) -> SessionState {
        self.cancel_stroke();
        let Some(edit) = self.redo_stack.pop() else {
            return self.state();
        };
        match &edit {
            Edit::AddStroke(stroke) => self.document.strokes.push(stroke.clone()),
            Edit::Clear(_) => self.document.strokes.clear(),
        }
        self.undo_stack.push(edit);
        self.bump()
    }

    pub fn replace_document(&mut self, document: Document) -> SessionState {
        self.cancel_stroke();
        self.document = document;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.bump()
    }

    /// Starts a stroke, cancelling any stroke already live. `view_to_doc` must be a uniform
    /// scale plus translation.
    pub fn begin_stroke(
        &mut self,
        pen: Pen,
        view_to_doc: Affine,
        sample: InputSample,
    ) -> LiveUpdate {
        let mut builder = StrokeBuilder::begin(pen, view_to_doc, sample);
        let update = builder.live_update(&[]);
        self.live = Some(builder);
        update
    }

    pub fn extend_stroke(
        &mut self,
        samples: &[InputSample],
        predicted: &[InputSample],
    ) -> LiveUpdate {
        let Some(builder) = self.live.as_mut() else {
            return LiveUpdate::default();
        };
        builder.extend(samples);
        builder.live_update(predicted)
    }

    /// Commits the live stroke and returns its full outline.
    pub fn end_stroke(&mut self) -> Option<(SessionState, PathData)> {
        let stroke = self.live.take()?.finish();
        let path = outline_stroke(&stroke);
        Some((self.add_stroke(stroke), path))
    }

    pub fn cancel_stroke(&mut self) {
        self.live = None;
    }

    pub fn has_live_stroke(&self) -> bool {
        self.live.is_some()
    }

    fn record(&mut self, edit: Edit) -> SessionState {
        self.undo_stack.push(edit);
        self.redo_stack.clear();
        self.bump()
    }

    fn bump(&mut self) -> SessionState {
        self.revision += 1;
        self.state()
    }
}
