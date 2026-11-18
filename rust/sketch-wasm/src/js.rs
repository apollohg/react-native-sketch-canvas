//! JavaScript shapes of the sketch-core records, declared once for TypeScript and wasm-bindgen.

use js_sys::{Float32Array, Object, Uint8Array};
use sketch_core::{
    Affine, Color, InputSample, LiveUpdate, PathData, Pen, PointerKind, SessionState,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub const POINTER_KIND_TOUCH: &str = "touch";
pub const POINTER_KIND_STYLUS: &str = "stylus";

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_RECORDS: &str = r#"
export interface SessionState {
    strokeCount: number;
    canUndo: boolean;
    canRedo: boolean;
    revision: number;
}

export interface Size {
    width: number;
    height: number;
}

/** SVG matrix order: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`. */
export interface Affine {
    a: number;
    b: number;
    c: number;
    d: number;
    e: number;
    f: number;
}

/** `color` is `#RRGGBB` or `#RRGGBBAA`; widths are in view points. */
export interface Pen {
    color: string;
    minWidth: number;
    maxWidth: number;
}

export type PointerKind = 'touch' | 'stylus';

/** One pointer sample in view points. */
export interface InputSample {
    x: number;
    y: number;
    timeMs: number;
    pressure?: number;
    kind: PointerKind;
}

/** Fill with the "nonzero" rule; each `PathVerb` consumes 2, 2, 4, 6 or 0 coords. */
export interface PathData {
    verbs: Uint8Array;
    coords: Float32Array;
}

/** Document units. `settled` extends what earlier updates sent; `tail` replaces the previous tail. */
export interface LiveUpdate {
    settled: PathData;
    tail: PathData;
}

export interface StrokeCommit {
    state: SessionState;
    path: PathData;
}

/** `color` is `#RRGGBB`, or `#RRGGBBAA` when not opaque. */
export interface CommittedStroke {
    path: PathData;
    color: string;
}
"#;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("invalid color {0:?}: expected #RRGGBB or #RRGGBBAA")]
    InvalidColor(String),
    #[error(
        "invalid pointer kind {0:?}: expected \"{POINTER_KIND_TOUCH}\" or \"{POINTER_KIND_STYLUS}\""
    )]
    InvalidPointerKind(String),
}

pub fn parse_color(text: &str) -> Result<Color, InputError> {
    Color::parse(text).ok_or_else(|| InputError::InvalidColor(text.to_string()))
}

pub fn parse_pointer_kind(text: &str) -> Result<PointerKind, InputError> {
    match text {
        POINTER_KIND_TOUCH => Ok(PointerKind::Touch),
        POINTER_KIND_STYLUS => Ok(PointerKind::Stylus),
        _ => Err(InputError::InvalidPointerKind(text.to_string())),
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Pen")]
    pub type JsPen;
    #[wasm_bindgen(method, getter)]
    fn color(this: &JsPen) -> String;
    #[wasm_bindgen(method, getter = minWidth)]
    fn min_width(this: &JsPen) -> f64;
    #[wasm_bindgen(method, getter = maxWidth)]
    fn max_width(this: &JsPen) -> f64;

    #[wasm_bindgen(typescript_type = "InputSample")]
    pub type JsInputSample;
    #[wasm_bindgen(method, getter)]
    fn x(this: &JsInputSample) -> f64;
    #[wasm_bindgen(method, getter)]
    fn y(this: &JsInputSample) -> f64;
    #[wasm_bindgen(method, getter = timeMs)]
    fn time_ms(this: &JsInputSample) -> f64;
    #[wasm_bindgen(method, getter)]
    fn pressure(this: &JsInputSample) -> Option<f64>;
    #[wasm_bindgen(method, getter)]
    fn kind(this: &JsInputSample) -> String;

    #[wasm_bindgen(typescript_type = "Affine")]
    pub type JsAffine;
    #[wasm_bindgen(method, getter)]
    fn a(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, getter)]
    fn b(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, getter)]
    fn c(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, getter)]
    fn d(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, getter)]
    fn e(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, getter)]
    fn f(this: &JsAffine) -> f64;
    #[wasm_bindgen(method, setter = a)]
    fn set_a(this: &JsAffine, value: f64);
    #[wasm_bindgen(method, setter = b)]
    fn set_b(this: &JsAffine, value: f64);
    #[wasm_bindgen(method, setter = c)]
    fn set_c(this: &JsAffine, value: f64);
    #[wasm_bindgen(method, setter = d)]
    fn set_d(this: &JsAffine, value: f64);
    #[wasm_bindgen(method, setter = e)]
    fn set_e(this: &JsAffine, value: f64);
    #[wasm_bindgen(method, setter = f)]
    fn set_f(this: &JsAffine, value: f64);

    #[wasm_bindgen(typescript_type = "SessionState")]
    pub type JsSessionState;
    #[wasm_bindgen(method, setter = strokeCount)]
    fn set_stroke_count(this: &JsSessionState, value: u32);
    #[wasm_bindgen(method, setter = canUndo)]
    fn set_can_undo(this: &JsSessionState, value: bool);
    #[wasm_bindgen(method, setter = canRedo)]
    fn set_can_redo(this: &JsSessionState, value: bool);
    #[wasm_bindgen(method, setter = revision)]
    fn set_revision(this: &JsSessionState, value: f64);

    #[wasm_bindgen(typescript_type = "Size")]
    pub type JsSize;
    #[wasm_bindgen(method, setter = width)]
    fn set_width(this: &JsSize, value: f64);
    #[wasm_bindgen(method, setter = height)]
    fn set_height(this: &JsSize, value: f64);

    #[wasm_bindgen(typescript_type = "PathData")]
    pub type JsPathData;
    #[wasm_bindgen(method, setter = verbs)]
    fn set_verbs(this: &JsPathData, value: &Uint8Array);
    #[wasm_bindgen(method, setter = coords)]
    fn set_coords(this: &JsPathData, value: &Float32Array);

    #[wasm_bindgen(typescript_type = "LiveUpdate")]
    pub type JsLiveUpdate;
    #[wasm_bindgen(method, setter = settled)]
    fn set_settled(this: &JsLiveUpdate, value: &JsPathData);
    #[wasm_bindgen(method, setter = tail)]
    fn set_tail(this: &JsLiveUpdate, value: &JsPathData);

    #[wasm_bindgen(typescript_type = "StrokeCommit")]
    pub type JsStrokeCommit;
    #[wasm_bindgen(method, setter = state)]
    fn set_state(this: &JsStrokeCommit, value: &JsSessionState);
    #[wasm_bindgen(method, setter = path)]
    fn set_path(this: &JsStrokeCommit, value: &JsPathData);

    #[wasm_bindgen(typescript_type = "CommittedStroke")]
    pub type JsCommittedStroke;
    #[wasm_bindgen(method, setter = path)]
    fn set_path(this: &JsCommittedStroke, value: &JsPathData);
    #[wasm_bindgen(method, setter = color)]
    fn set_color(this: &JsCommittedStroke, value: &str);
}

fn new_record<T: JsCast>() -> T {
    Object::new().unchecked_into()
}

pub fn pen_from_js(pen: &JsPen) -> Result<Pen, InputError> {
    Ok(Pen {
        color: parse_color(&pen.color())?,
        min_width: pen.min_width(),
        max_width: pen.max_width(),
    })
}

pub fn sample_from_js(sample: &JsInputSample) -> Result<InputSample, InputError> {
    Ok(InputSample {
        x: sample.x(),
        y: sample.y(),
        time_ms: sample.time_ms(),
        pressure: sample.pressure(),
        kind: parse_pointer_kind(&sample.kind())?,
    })
}

pub fn samples_from_js(samples: &[JsInputSample]) -> Result<Vec<InputSample>, InputError> {
    samples.iter().map(sample_from_js).collect()
}

pub fn affine_from_js(affine: &JsAffine) -> Affine {
    Affine {
        a: affine.a(),
        b: affine.b(),
        c: affine.c(),
        d: affine.d(),
        e: affine.e(),
        f: affine.f(),
    }
}

pub fn affine_to_js(affine: Affine) -> JsAffine {
    let record: JsAffine = new_record();
    record.set_a(affine.a);
    record.set_b(affine.b);
    record.set_c(affine.c);
    record.set_d(affine.d);
    record.set_e(affine.e);
    record.set_f(affine.f);
    record
}

pub fn state_to_js(state: SessionState) -> JsSessionState {
    let record: JsSessionState = new_record();
    record.set_stroke_count(state.stroke_count);
    record.set_can_undo(state.can_undo);
    record.set_can_redo(state.can_redo);
    record.set_revision(state.revision as f64);
    record
}

pub fn size_to_js(width: f64, height: f64) -> JsSize {
    let record: JsSize = new_record();
    record.set_width(width);
    record.set_height(height);
    record
}

pub fn path_to_js(path: &PathData) -> JsPathData {
    let record: JsPathData = new_record();
    record.set_verbs(&Uint8Array::from(path.verbs.as_slice()));
    record.set_coords(&Float32Array::from(path.coords.as_slice()));
    record
}

pub fn live_update_to_js(update: &LiveUpdate) -> JsLiveUpdate {
    let record: JsLiveUpdate = new_record();
    record.set_settled(&path_to_js(&update.settled));
    record.set_tail(&path_to_js(&update.tail));
    record
}

pub fn stroke_commit_to_js(state: SessionState, path: &PathData) -> JsStrokeCommit {
    let record: JsStrokeCommit = new_record();
    record.set_state(&state_to_js(state));
    record.set_path(&path_to_js(path));
    record
}

pub fn committed_stroke_to_js(path: &PathData, color: Color) -> JsCommittedStroke {
    let record: JsCommittedStroke = new_record();
    record.set_path(&path_to_js(path));
    record.set_color(&color.to_hex());
    record
}
