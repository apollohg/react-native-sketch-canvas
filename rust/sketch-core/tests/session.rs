use sketch_core::{Color, Document, Point, Session, Stroke};

fn stroke(x: f64) -> Stroke {
    Stroke {
        color: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        points: vec![Point { x, y: 1.0, w: 2.0 }],
    }
}

fn session() -> Session {
    Session::new(Document::new(100.0, 50.0).expect("valid size"))
}

#[test]
fn add_then_undo_redo() {
    let mut s = session();
    s.add_stroke(stroke(1.0));
    let state = s.add_stroke(stroke(2.0));
    assert_eq!(state.stroke_count, 2, "{state:?}");
    let state = s.undo();
    assert!(state.stroke_count == 1 && state.can_redo, "{state:?}");
    let state = s.redo();
    assert!(state.stroke_count == 2 && !state.can_redo, "{state:?}");
}

#[test]
fn add_after_undo_drops_redo() {
    let mut s = session();
    s.add_stroke(stroke(1.0));
    s.undo();
    let state = s.add_stroke(stroke(2.0));
    assert!(!state.can_redo, "{state:?}");
    assert_eq!(s.document().strokes, vec![stroke(2.0)]);
}

#[test]
fn clear_is_undoable() {
    let mut s = session();
    s.add_stroke(stroke(1.0));
    s.add_stroke(stroke(2.0));
    let state = s.clear();
    assert_eq!(state.stroke_count, 0, "{state:?}");
    s.undo();
    assert_eq!(s.document().strokes, vec![stroke(1.0), stroke(2.0)]);
}

#[test]
fn clear_on_empty_is_not_recorded() {
    let mut s = session();
    let state = s.clear();
    assert!(!state.can_undo && state.revision == 0, "{state:?}");
}

#[test]
fn revision_counts_applied_changes() {
    let mut s = session();
    s.add_stroke(stroke(1.0));
    s.add_stroke(stroke(2.0));
    s.undo();
    s.redo();
    let state = s.clear();
    assert_eq!(state.revision, 5, "{state:?}");
    let mut fresh = session();
    let state = fresh.undo();
    assert_eq!(state.revision, 0, "undo with empty history: {state:?}");
    let state = fresh.redo();
    assert_eq!(state.revision, 0, "redo with empty history: {state:?}");
}

#[test]
fn replace_document_resets_history() {
    let mut s = session();
    s.add_stroke(stroke(1.0));
    let mut doc2 = Document::new(10.0, 10.0).expect("valid size");
    doc2.strokes.push(stroke(9.0));
    let state = s.replace_document(doc2.clone());
    assert!(!state.can_undo && !state.can_redo, "{state:?}");
    assert_eq!(s.document().strokes, doc2.strokes);
    assert_eq!(state.revision, 2, "{state:?}");
}
