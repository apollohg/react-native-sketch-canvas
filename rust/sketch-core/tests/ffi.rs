#![cfg(feature = "ffi")]

use sketch_core::ffi::{SketchError, SketchSession};
use sketch_core::{
    parse_document, serialize_document, Affine, Color, InputSample, Pen, PointerKind,
};

const FIXTURE: &str = include_str!("../../../fixtures/documents/two-strokes.json");

fn sample(x: f64) -> InputSample {
    InputSample {
        x,
        y: 10.0,
        time_ms: x,
        pressure: None,
        kind: PointerKind::Touch,
    }
}

#[test]
fn session_round_trips_json() {
    let session = SketchSession::from_json(FIXTURE.to_string()).expect("fixture loads");
    let expected = serialize_document(&parse_document(FIXTURE).expect("fixture parses"));
    assert_eq!(session.to_json(), expected);
}

#[test]
fn invalid_json_maps_to_invalid_document() {
    match SketchSession::from_json("{}".to_string()) {
        Err(SketchError::InvalidDocument { message }) => {
            assert!(message.contains("version"), "message: {message}")
        }
        other => panic!(
            "expected InvalidDocument, got {:?}",
            other.map(|s| s.to_json())
        ),
    }
}

#[test]
fn stroke_lifecycle_through_ffi() {
    let session = SketchSession::new_empty(100.0, 100.0).expect("valid size");
    let pen = Pen {
        color: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        min_width: 1.0,
        max_width: 3.0,
    };
    session.begin_stroke(pen, Affine::IDENTITY, sample(0.0));
    session.extend_stroke(vec![sample(20.0), sample(40.0)], vec![sample(50.0)]);
    let commit = session.end_stroke().expect("live stroke commits");
    assert_eq!(commit.state.stroke_count, 1, "{:?}", commit.state);
    assert!(!commit.path.verbs.is_empty(), "committed outline is empty");
    assert_eq!(session.committed_paths().len(), 1);
}

#[test]
fn load_json_rejects_and_keeps_document() {
    let session = SketchSession::from_json(FIXTURE.to_string()).expect("fixture loads");
    let before = session.to_json();
    let result =
        session.load_json(r#"{"version":1,"width":0,"height":1,"strokes":[]}"#.to_string());
    assert!(
        matches!(result, Err(SketchError::InvalidDocument { .. })),
        "{result:?}"
    );
    assert_eq!(session.to_json(), before);
}
