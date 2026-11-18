mod common;

use common::{fixtures_dir, load_fixture_document, rasterize};
use sketch_core::{
    export_svg, fit_transform, outline_stroke, parse_background, parse_document, Document, DrawOp,
};

const TWO_STROKES: &str = r##"{"version":1,"width":300,"height":150,"strokes":[
    {"color":"#FF0000","points":[[10,20,3],[40,60,3],[80,90,3]]},
    {"color":"#0000FF80","points":[[150,75,4]]}]}"##;

fn parse_svg(svg: &str) -> resvg::usvg::Tree {
    resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .unwrap_or_else(|e| panic!("{e}\n{svg}"))
}

fn render(svg: &str, width: u32, height: u32) -> resvg::tiny_skia::Pixmap {
    let tree = parse_svg(svg);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).expect("pixmap");
    let scale = width as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
}

fn paths(group: &resvg::usvg::Group, out: &mut Vec<(resvg::usvg::Color, f32)>) {
    for node in group.children() {
        match node {
            resvg::usvg::Node::Group(g) => paths(g, out),
            resvg::usvg::Node::Path(p) => {
                let fill = p.fill().expect("stroke path has a fill");
                let resvg::usvg::Paint::Color(color) = fill.paint() else {
                    panic!("non-solid fill")
                };
                out.push((*color, fill.opacity().get()));
            }
            _ => {}
        }
    }
}

fn doc(json: &str) -> Document {
    parse_document(json).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn exports_one_path_per_stroke() {
    let svg = export_svg(&doc(TWO_STROKES), None).expect("export");
    let mut found = Vec::new();
    paths(parse_svg(&svg).root(), &mut found);
    assert_eq!(found.len(), 2, "{svg}");
    assert_eq!(found[0].0, resvg::usvg::Color::new_rgb(255, 0, 0), "{svg}");
    assert_eq!(found[0].1, 1.0, "{svg}");
    assert_eq!(found[1].0, resvg::usvg::Color::new_rgb(0, 0, 255), "{svg}");
    assert!(
        (found[1].1 - 0.5).abs() < 0.01,
        "opacity {}: {svg}",
        found[1].1
    );
}

#[test]
fn root_has_document_size() {
    let svg = export_svg(&doc(TWO_STROKES), None).expect("export");
    let xml =
        resvg::usvg::roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    let root = xml.root_element();
    let attrs = (
        root.attribute("width"),
        root.attribute("height"),
        root.attribute("viewBox"),
    );
    assert_eq!(
        attrs,
        (Some("300"), Some("150"), Some("0 0 300 150")),
        "{svg}"
    );
}

#[test]
fn background_is_aspect_fit() {
    let background = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="#0000FF"/></svg>"##;
    let svg = export_svg(
        &doc(r#"{"version":1,"width":100,"height":100,"strokes":[]}"#),
        Some(background),
    )
    .expect("export");
    let pixmap = render(&svg, 100, 100);
    let at = |x: u32, y: u32| pixmap.pixel(x, y).expect("pixel");
    assert_eq!(
        at(50, 10).alpha(),
        0,
        "letterbox pixel (50, 10) is painted\n{svg}"
    );
    let centre = at(50, 50);
    assert_eq!(
        (centre.red(), centre.green(), centre.blue(), centre.alpha()),
        (0, 0, 255, 255),
        "centre pixel\n{svg}"
    );
}

#[test]
fn background_prolog_is_stripped() {
    let background = "<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">\n<!-- exported -->\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\"/></svg>\n<!-- trailing -->";
    let svg = export_svg(&doc(TWO_STROKES), Some(background)).expect("export");
    assert!(
        !svg.contains("<?xml") && !svg.contains("DOCTYPE") && !svg.contains("<!--"),
        "{svg}"
    );
    parse_svg(&svg);
}

#[test]
fn export_matches_outline_coverage() {
    let hairpin = load_fixture_document("conformance/hairpin.json");
    let shifted = Document {
        width: 140.0,
        height: 60.0,
        strokes: hairpin
            .strokes
            .iter()
            .map(|s| sketch_core::Stroke {
                color: s.color,
                points: s
                    .points
                    .iter()
                    .map(|p| sketch_core::Point {
                        x: p.x + 10.0,
                        y: p.y + 20.0,
                        w: p.w,
                    })
                    .collect(),
            })
            .collect(),
    };
    let svg = export_svg(&shifted, None).expect("export");
    let exported = render(&svg, 140, 60);
    let reference = rasterize(&outline_stroke(&shifted.strokes[0]), 0.0);
    let mut differing = 0;
    let mut total = 0;
    for y in 0..60u32 {
        for x in 0..140u32 {
            let expected = reference.coverage_at_doc(x as f32 + 0.5, y as f32 + 0.5);
            let actual = f32::from(exported.pixel(x, y).expect("pixel").alpha()) / 255.0;
            total += 1;
            if (expected - actual).abs() > 0.1 {
                differing += 1;
            }
        }
    }
    let ratio = differing as f64 / total as f64;
    assert!(
        ratio <= 0.01,
        "{differing} of {total} pixels differ by > 0.1 coverage ({:.2}%)",
        ratio * 100.0
    );
}

#[test]
fn invalid_background_errors() {
    let invalid =
        std::fs::read_to_string(fixtures_dir().join("backgrounds/invalid.svg")).expect("fixture");
    assert!(export_svg(&doc(TWO_STROKES), Some(&invalid)).is_err());
}

#[test]
fn background_entities_survive_export() {
    let background = "<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\" [\n<!ENTITY blue \"#0000FF\">\n]>\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\" fill=\"&blue;\"/></svg>";
    let svg = export_svg(
        &doc(r#"{"version":1,"width":10,"height":10,"strokes":[]}"#),
        Some(background),
    )
    .expect("export");
    assert!(
        !svg.contains("<!DOCTYPE") && !svg.contains("<!ENTITY"),
        "export re-emits the DTD: {svg}"
    );
    let pixmap = render(&svg, 10, 10);
    let centre = pixmap.pixel(5, 5).expect("pixel");
    assert_eq!(
        (centre.red(), centre.green(), centre.blue(), centre.alpha()),
        (0, 0, 255, 255),
        "{svg}"
    );
}

#[test]
fn background_markup_is_escaped_on_export() {
    let background = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10"><title>a &lt; b &amp; "c"</title><rect id="r&quot;" width="10" height="10"/><use xlink:href="#r&quot;"/></svg>"##;
    let svg = export_svg(
        &doc(r#"{"version":1,"width":10,"height":10,"strokes":[]}"#),
        Some(background),
    )
    .expect("export");
    let xml =
        resvg::usvg::roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    let title = xml
        .descendants()
        .find(|n| n.has_tag_name("title"))
        .and_then(|n| n.text());
    assert_eq!(title, Some(r#"a < b & "c""#), "{svg}");
    let href = xml
        .descendants()
        .find(|n| n.has_tag_name("use"))
        .and_then(|n| n.attribute(("http://www.w3.org/1999/xlink", "href")));
    assert_eq!(href, Some("#r\""), "{svg}");
}

#[test]
fn background_namespaces_survive_export() {
    let background = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" xmlns:a0="urn:other" viewBox="0 0 10 10"><g s:class="k"><a0:meta/></g></svg>"#;
    let svg = export_svg(
        &doc(r#"{"version":1,"width":10,"height":10,"strokes":[]}"#),
        Some(background),
    )
    .expect("export");
    let xml =
        resvg::usvg::roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    let meta = xml
        .descendants()
        .find(|n| n.tag_name().name() == "meta")
        .unwrap_or_else(|| panic!("{svg}"));
    assert_eq!(meta.tag_name().namespace(), Some("urn:other"), "{svg}");
    let group = xml
        .descendants()
        .find(|n| n.has_tag_name("g"))
        .unwrap_or_else(|| panic!("{svg}"));
    assert_eq!(
        group.attribute(("http://www.w3.org/2000/svg", "class")),
        Some("k"),
        "{svg}"
    );
}

/// Bounds of the pixels at least half covered, as (min x, min y, max x, max y) pixel edges.
fn painted_bounds(pixmap: &resvg::tiny_skia::Pixmap) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for y in 0..pixmap.height() {
        for x in 0..pixmap.width() {
            if pixmap.pixel(x, y).expect("pixel").alpha() >= 128 {
                let (x0, y0, x1, y1) = bounds.unwrap_or((x, y, x + 1, y + 1));
                bounds = Some((x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)));
            }
        }
    }
    bounds
}

/// Where the canvas draws the background's one rect: its ops fitted into the document.
fn drawn_bounds(background: &str, doc_width: f64, doc_height: f64) -> (u32, u32, u32, u32) {
    let parsed = parse_background(background).expect("background");
    let fit = fit_transform(parsed.width, parsed.height, doc_width, doc_height);
    let [DrawOp::FillPath {
        path, transform, ..
    }] = parsed.ops.as_slice()
    else {
        panic!("expected one fill, got {:?}", parsed.ops)
    };
    let to_doc = transform.then(&fit);
    let points: Vec<(f64, f64)> = path
        .coords
        .chunks(2)
        .map(|xy| to_doc.apply(f64::from(xy[0]), f64::from(xy[1])))
        .collect();
    let edge = |values: Vec<f64>, max: bool| {
        let v = values
            .into_iter()
            .fold(if max { f64::MIN } else { f64::MAX }, |a, b| {
                if max {
                    a.max(b)
                } else {
                    a.min(b)
                }
            });
        v.round() as u32
    };
    (
        edge(points.iter().map(|p| p.0).collect(), false),
        edge(points.iter().map(|p| p.1).collect(), false),
        edge(points.iter().map(|p| p.0).collect(), true),
        edge(points.iter().map(|p| p.1).collect(), true),
    )
}

#[test]
fn exported_background_is_placed_where_the_canvas_draws_it() {
    let cases = [
        r#"width="200" height="100" viewBox="0 0 100 100""#,
        r#"width="200" height="100" viewBox="0 0 100 100" preserveAspectRatio="xMaxYMax meet""#,
        r#"width="200" height="100" viewBox="0 0 100 100" preserveAspectRatio="none""#,
        r#"width="40mm" height="20mm" viewBox="0 0 100 100""#,
        r#"x="30" y="30" width="200" height="100""#,
    ];
    for root in cases {
        let background = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" {root}><rect width="100" height="100" fill="#0000FF"/></svg>"##
        );
        let svg = export_svg(
            &doc(r#"{"version":1,"width":100,"height":200,"strokes":[]}"#),
            Some(&background),
        )
        .expect("export");
        let expected = drawn_bounds(&background, 100.0, 200.0);
        let exported = painted_bounds(&render(&svg, 100, 200));
        assert_eq!(
            exported,
            Some(expected),
            "background <svg {root}>: export paints {exported:?}, the canvas draws {expected:?}\n{svg}"
        );
    }
}

#[test]
fn deeply_nested_background_markup_exports() {
    const DEPTH: usize = 100_000;
    let background = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="5" height="5"/><metadata>{}{}</metadata></svg>"#,
        "<x>".repeat(DEPTH),
        "</x>".repeat(DEPTH)
    );
    let svg = export_svg(
        &doc(r#"{"version":1,"width":10,"height":10,"strokes":[]}"#),
        Some(&background),
    )
    .expect("export");
    assert_eq!(svg.matches("<x").count(), DEPTH, "nested elements lost");
}
