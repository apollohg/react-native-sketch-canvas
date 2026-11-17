//! Regenerates `fixtures/conformance/*.outline.json` from each `*.json` input document.

use std::path::Path;

use sketch_core::{outline_stroke, parse_document};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/conformance");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("read conformance dir")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            name.ends_with(".json") && !name.ends_with(".outline.json")
        })
        .collect();
    entries.sort();
    for input in entries {
        let json = std::fs::read_to_string(&input).expect("read input");
        let doc = parse_document(&json).unwrap_or_else(|e| panic!("{}: {e}", input.display()));
        let strokes: Vec<_> = doc
            .strokes
            .iter()
            .map(|stroke| {
                let path = outline_stroke(stroke);
                serde_json::json!({ "verbs": path.verbs, "coords": path.coords })
            })
            .collect();
        let output = input.with_extension("outline.json");
        let text =
            serde_json::to_string(&serde_json::json!({ "strokes": strokes })).expect("serialise");
        std::fs::write(&output, text + "\n").expect("write outline");
        println!("wrote {}", output.display());
    }
}
