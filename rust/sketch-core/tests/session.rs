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
    let state = s
        .add_stroke(stroke(2.0))
        .expect("within the document limits");
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
    let state = s
        .add_stroke(stroke(2.0))
        .expect("within the document limits");
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

#[test]
fn commands_cancel_live_stroke() {
    use sketch_core::{Affine, InputSample, Pen, PointerKind};
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
    let sample = |x: f64| InputSample {
        x,
        y: 10.0,
        time_ms: x,
        pressure: None,
        kind: PointerKind::Touch,
    };
    type Command = (&'static str, fn(&mut Session));
    let commands: [Command; 4] = [
        ("undo", |s| {
            s.undo();
        }),
        ("redo", |s| {
            s.redo();
        }),
        ("clear", |s| {
            s.clear();
        }),
        ("replace_document", |s| {
            let doc = s.document().clone();
            s.replace_document(doc);
        }),
    ];
    for (name, command) in commands {
        let mut s = session();
        s.add_stroke(stroke(1.0));
        s.add_stroke(stroke(2.0));
        s.undo();
        let before = s.document().strokes.clone();
        s.begin_stroke(pen, Affine::IDENTITY, sample(0.0));
        s.extend_stroke(&[sample(20.0), sample(40.0)], &[]);
        command(&mut s);
        assert!(!s.has_live_stroke(), "{name}: live stroke survived");
        assert!(
            s.end_stroke().is_none(),
            "{name}: end_stroke committed a half-stroke"
        );
        let after = &s.document().strokes;
        assert!(after.len() <= 2, "{name}: strokes {after:?}");
        assert!(
            after
                .iter()
                .all(|st| before.contains(st) || *st == stroke(2.0)),
            "{name}: half-stroke leaked into {after:?}"
        );
    }
}
