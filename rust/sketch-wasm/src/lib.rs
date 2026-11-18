//! The wasm-bindgen surface used by the web editor, mirroring `sketch_core::ffi`. Thin wrappers only.

mod js;

use sketch_core::{
    outline_stroke, parse_document, serialize_document, Document, Session, VERB_CLOSE, VERB_CUBIC,
    VERB_LINE, VERB_MOVE, VERB_QUAD,
};
use wasm_bindgen::prelude::*;

pub use js::{parse_color, parse_pointer_kind, InputError};
use js::{
    JsAffine, JsCommittedStroke, JsInputSample, JsLiveUpdate, JsPen, JsSessionState, JsSize,
    JsStrokeCommit,
};

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathVerb {
    Move = 0,
    Line = 1,
    Quad = 2,
    Cubic = 3,
    Close = 4,
}

const _: () = assert!(
    PathVerb::Move as u8 == VERB_MOVE
        && PathVerb::Line as u8 == VERB_LINE
        && PathVerb::Quad as u8 == VERB_QUAD
        && PathVerb::Cubic as u8 == VERB_CUBIC
        && PathVerb::Close as u8 == VERB_CLOSE
);

#[wasm_bindgen]
pub struct SketchSession {
    session: Session,
}

#[wasm_bindgen]
impl SketchSession {
    #[wasm_bindgen(js_name = newEmpty)]
    pub fn new_empty(width: f64, height: f64) -> Result<SketchSession, JsError> {
        Ok(SketchSession {
            session: Session::new(Document::new(width, height)?),
        })
    }

    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<SketchSession, JsError> {
        Ok(SketchSession {
            session: Session::new(parse_document(json)?),
        })
    }

    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String {
        serialize_document(self.session.document())
    }

    /// Replaces the document and resets history; an invalid document leaves everything unchanged.
    #[wasm_bindgen(js_name = loadJson)]
    pub fn load_json(&mut self, json: &str) -> Result<JsSessionState, JsError> {
        let document = parse_document(json)?;
        Ok(js::state_to_js(self.session.replace_document(document)))
    }

    pub fn state(&self) -> JsSessionState {
        js::state_to_js(self.session.state())
    }

    #[wasm_bindgen(js_name = documentSize)]
    pub fn document_size(&self) -> JsSize {
        let document = self.session.document();
        js::size_to_js(document.width, document.height)
    }

    pub fn undo(&mut self) -> JsSessionState {
        js::state_to_js(self.session.undo())
    }

    pub fn redo(&mut self) -> JsSessionState {
        js::state_to_js(self.session.redo())
    }

    pub fn clear(&mut self) -> JsSessionState {
        js::state_to_js(self.session.clear())
    }

    /// Starts a stroke, cancelling any stroke already live. `viewToDoc` must be a uniform scale
    /// plus translation.
    #[wasm_bindgen(js_name = beginStroke)]
    pub fn begin_stroke(
        &mut self,
        pen: &JsPen,
        #[wasm_bindgen(js_name = viewToDoc)] view_to_doc: &JsAffine,
        sample: &JsInputSample,
    ) -> Result<JsLiveUpdate, JsError> {
        let update = self.session.begin_stroke(
            js::pen_from_js(pen)?,
            js::affine_from_js(view_to_doc),
            js::sample_from_js(sample)?,
        );
        Ok(js::live_update_to_js(&update))
    }

    #[wasm_bindgen(js_name = extendStroke)]
    pub fn extend_stroke(
        &mut self,
        samples: Vec<JsInputSample>,
        predicted: Vec<JsInputSample>,
    ) -> Result<JsLiveUpdate, JsError> {
        let update = self.session.extend_stroke(
            &js::samples_from_js(&samples)?,
            &js::samples_from_js(&predicted)?,
        );
        Ok(js::live_update_to_js(&update))
    }

    /// Commits the live stroke; `undefined` when there is none or it would take the document past
    /// the format's limits.
    #[wasm_bindgen(js_name = endStroke)]
    pub fn end_stroke(&mut self) -> Option<JsStrokeCommit> {
        self.session
            .end_stroke()
            .map(|(state, path)| js::stroke_commit_to_js(state, &path))
    }

    #[wasm_bindgen(js_name = cancelStroke)]
    pub fn cancel_stroke(&mut self) {
        self.session.cancel_stroke();
    }

    #[wasm_bindgen(js_name = hasLiveStroke)]
    pub fn has_live_stroke(&self) -> bool {
        self.session.has_live_stroke()
    }

    #[wasm_bindgen(js_name = committedPaths)]
    pub fn committed_paths(&self) -> Vec<JsCommittedStroke> {
        self.session
            .document()
            .strokes
            .iter()
            .map(|stroke| js::committed_stroke_to_js(&outline_stroke(stroke), stroke.color))
            .collect()
    }

    #[wasm_bindgen(js_name = exportSvg)]
    pub fn export_svg(&self, background: Option<String>) -> Result<String, JsError> {
        Ok(sketch_core::export_svg(
            self.session.document(),
            background.as_deref(),
        )?)
    }
}

#[wasm_bindgen(js_name = fitTransform)]
pub fn fit_transform(
    #[wasm_bindgen(js_name = contentWidth)] content_width: f64,
    #[wasm_bindgen(js_name = contentHeight)] content_height: f64,
    #[wasm_bindgen(js_name = boxWidth)] box_width: f64,
    #[wasm_bindgen(js_name = boxHeight)] box_height: f64,
) -> JsAffine {
    js::affine_to_js(sketch_core::fit_transform(
        content_width,
        content_height,
        box_width,
        box_height,
    ))
}
