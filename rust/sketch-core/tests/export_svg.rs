mod common;

use common::{fixtures_dir, load_fixture_document, rasterize};
use sketch_core::{export_svg, outline_stroke, parse_document, Document};

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
    let pixmap = render(&svg, 10, 10);
    let centre = pixmap.pixel(5, 5).expect("pixel");
    assert_eq!(
        (centre.red(), centre.green(), centre.blue(), centre.alpha()),
        (0, 0, 255, 255),
        "{svg}"
    );
}
