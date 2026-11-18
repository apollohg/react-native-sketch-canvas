use sketch_core::{parse_document, serialize_document, DocumentError};

const FIXTURE: &str = include_str!("../../../fixtures/documents/two-strokes.json");

fn doc_json(strokes: &str) -> String {
    format!(r#"{{"version":1,"width":100,"height":50,"strokes":{strokes}}}"#)
}

fn expect_err(json: &str, expected: DocumentError) {
    let result = parse_document(json);
    assert_eq!(
        result.as_ref().err(),
        Some(&expected),
        "input: {json}\nactual: {result:?}"
    );
}

#[test]
fn parses_minimal_document() {
    let json = doc_json("[]");
    let doc = parse_document(&json).unwrap_or_else(|e| panic!("input: {json}\nerror: {e:?}"));
    assert_eq!(
        (doc.width, doc.height, doc.strokes.len()),
        (100.0, 50.0, 0),
        "input: {json}\nactual: {doc:?}"
    );
}

#[test]
fn round_trip_is_stable() {
    let first = parse_document(FIXTURE).unwrap_or_else(|e| panic!("fixture error: {e:?}"));
    let s1 = serialize_document(&first);
    let s2 = serialize_document(
        &parse_document(&s1).unwrap_or_else(|e| panic!("s1: {s1}\nerror: {e:?}")),
    );
    assert_eq!(s1, s2, "fixture serialised differently on the second pass");
    assert_eq!(first.strokes.len(), 2, "s1: {s1}");
    assert_eq!(first.strokes[0].points.len(), 3, "s1: {s1}");
}

#[test]
fn rejects_unknown_version() {
    expect_err(
        r#"{"version":2,"width":100,"height":50,"strokes":[]}"#,
        DocumentError::UnsupportedVersion { found: 2 },
    );
}

#[test]
fn rejects_non_positive_size() {
    expect_err(
        r#"{"version":1,"width":0,"height":50,"strokes":[]}"#,
        DocumentError::InvalidSize,
    );
    expect_err(
        r#"{"version":1,"width":100,"height":-1,"strokes":[]}"#,
        DocumentError::InvalidSize,
    );
}

#[test]
fn rejects_bad_color() {
    for color in ["red", "#12345", "#GGGGGG"] {
        let json = doc_json(&format!(r#"[{{"color":"{color}","points":[[1,2,3]]}}]"#));
        expect_err(&json, DocumentError::InvalidColor { stroke: 0 });
    }
}

#[test]
fn rejects_empty_points() {
    expect_err(
        &doc_json(r##"[{"color":"#000000","points":[]}]"##),
        DocumentError::EmptyStroke { stroke: 0 },
    );
}

#[test]
fn rejects_non_positive_width() {
    for point in ["[1,2,0]", "[1,2,-1]"] {
        let json = doc_json(&format!(r##"[{{"color":"#000000","points":[{point}]}}]"##));
        expect_err(
            &json,
            DocumentError::InvalidWidth {
                stroke: 0,
                point: 0,
            },
        );
    }
}

#[test]
fn rejects_wrong_point_arity() {
    let json = doc_json(r##"[{"color":"#000000","points":[[1,2]]}]"##);
    let result = parse_document(&json);
    assert!(
        matches!(result, Err(DocumentError::Malformed { .. })),
        "input: {json}\nactual: {result:?}"
    );
}

#[test]
fn rejects_unknown_fields() {
    let json = r#"{"version":1,"width":100,"height":50,"strokes":[],"layers":[]}"#;
    let result = parse_document(json);
    assert!(
        matches!(result, Err(DocumentError::Malformed { .. })),
        "input: {json}\nactual: {result:?}"
    );
}

#[test]
fn normalises_color_output() {
    for (input, expected) in [
        ("#ff0000", "#FF0000"),
        ("#FF0000FF", "#FF0000"),
        ("#FF000080", "#FF000080"),
    ] {
        let json = doc_json(&format!(r#"[{{"color":"{input}","points":[[1,2,3]]}}]"#));
        let out = serialize_document(
            &parse_document(&json).unwrap_or_else(|e| panic!("input: {json}\nerror: {e:?}")),
        );
        assert!(
            out.contains(&format!(r#""color":"{expected}""#)),
            "input: {json}\noutput: {out}"
        );
    }
}

#[test]
fn rounds_coordinates_to_two_decimals() {
    let json = doc_json(r##"[{"color":"#000000","points":[[1.23456,2.0,3.005]]}]"##);
    let out = serialize_document(
        &parse_document(&json).unwrap_or_else(|e| panic!("input: {json}\nerror: {e:?}")),
    );
    assert!(out.contains("[1.23,2,"), "input: {json}\noutput: {out}");
}

#[test]
fn accepts_points_outside_bounds() {
    let json = doc_json(r##"[{"color":"#000000","points":[[-10,500,2]]}]"##);
    let result = parse_document(&json);
    assert!(result.is_ok(), "input: {json}\nactual: {result:?}");
}

#[test]
fn rejects_sizes_and_points_past_the_limits() {
    expect_err(
        r#"{"version":1,"width":1000001,"height":50,"strokes":[]}"#,
        DocumentError::InvalidSize,
    );
    for point in [
        "[1000001,2,3]",
        "[1,-1000001,3]",
        "[1,2,1000001]",
        "[9e307,2,3]",
    ] {
        let json = doc_json(&format!(
            r##"[{{"color":"#000000","points":[[1,2,3],{point}]}}]"##
        ));
        expect_err(
            &json,
            DocumentError::InvalidWidth {
                stroke: 0,
                point: 1,
            },
        );
    }
    let json = doc_json(r##"[{"color":"#000000","points":[[1000000,-1000000,1000000]]}]"##);
    assert!(parse_document(&json).is_ok(), "input: {json}");
}

/// Points 200 units apart in a zigzag of hairline width: every quadratic flattens into 64 steps.
fn hairline_zigzag(count: usize) -> String {
    let points: Vec<String> = (0..count)
        .map(|i| format!("[{},{},0.01]", (i % 2) * 200, i))
        .collect();
    doc_json(&format!(
        r##"[{{"color":"#000000","points":[{}]}}]"##,
        points.join(",")
    ))
}

#[test]
fn rejects_documents_past_the_sample_limit() {
    let within = hairline_zigzag(15_000);
    assert!(
        parse_document(&within).is_ok(),
        "15,000 hairline zigzag points (about 960,000 samples) rejected"
    );
    let past = hairline_zigzag(16_000);
    let result = parse_document(&past);
    assert!(
        matches!(result, Err(DocumentError::TooLarge { .. })),
        "16,000 hairline zigzag points (about 1,024,000 samples) gave {:?}",
        result.map(|doc| doc.strokes[0].points.len())
    );
}

#[test]
fn rejects_documents_past_the_point_limit() {
    let json = doc_json(&format!(
        r##"[{{"color":"#000000","points":[{}]}}]"##,
        vec!["[1,1,1]"; 1_000_001].join(",")
    ));
    let result = parse_document(&json);
    assert!(
        matches!(result, Err(DocumentError::TooLarge { .. })),
        "1,000,001 identical points gave {:?}",
        result.map(|doc| doc.strokes[0].points.len())
    );
}
