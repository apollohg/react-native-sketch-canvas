mod common;

use common::fixtures_dir;
use sketch_core::{
    fit_transform, parse_background, Affine, Background, BackgroundError, BackgroundWarning, Color,
    DrawOp, ImageFormat, Paint,
};

const FIXTURES: [&str; 9] = [
    "paths",
    "transforms",
    "nested-image",
    "clip",
    "gradient",
    "raster-image",
    "unsupported",
    "external-href",
    "viewbox",
];

fn read(name: &str) -> String {
    let path = fixtures_dir().join(format!("backgrounds/{name}.svg"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load(name: &str) -> Background {
    parse_background(&read(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn solid_fills(background: &Background) -> Vec<Color> {
    background
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillPath {
                paint: Paint::Solid(color),
                ..
            } => Some(*color),
            _ => None,
        })
        .collect()
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

#[test]
fn size_comes_from_viewbox() {
    let bg = load("viewbox");
    assert!(
        (bg.width - 5536.89).abs() < 1e-2 && (bg.height - 5000.0).abs() < 1e-2,
        "size {} × {}",
        bg.width,
        bg.height
    );
}

#[test]
fn paths_become_fill_ops() {
    let bg = load("paths");
    assert_eq!(
        solid_fills(&bg),
        vec![rgb(255, 0, 0), rgb(0, 255, 0), rgb(0, 0, 255)],
        "ops: {:?}",
        bg.ops
    );
}

#[test]
fn transforms_are_carried() {
    let bg = load("transforms");
    let Some(DrawOp::FillPath {
        path, transform, ..
    }) = bg
        .ops
        .iter()
        .find(|op| matches!(op, DrawOp::FillPath { .. }))
    else {
        panic!("no fill op: {:?}", bg.ops);
    };
    let (x, y) = transform.apply(f64::from(path.coords[0]), f64::from(path.coords[1]));
    assert!(
        (x - 12.0).abs() < 1e-4 && (y - 22.0).abs() < 1e-4,
        "local (1, 1) mapped to ({x}, {y}); transform {transform:?}"
    );
}

#[test]
fn nested_svg_images_are_flattened() {
    let bg = load("nested-image");
    let fills = solid_fills(&bg);
    assert!(
        fills.contains(&rgb(255, 0, 0)) && fills.contains(&rgb(0, 0, 255)),
        "fills {fills:?}"
    );
    assert!(
        !bg.ops.iter().any(|op| matches!(op, DrawOp::Image { .. })),
        "SVG data became an Image op: {:?}",
        bg.ops
    );
}

#[test]
fn raster_image_stays_encoded() {
    let bg = load("raster-image");
    let images: Vec<_> = bg
        .ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Image { .. }))
        .collect();
    assert_eq!(images.len(), 1, "ops: {:?}", bg.ops);
    let DrawOp::Image { data, format, .. } = images[0] else {
        unreachable!()
    };
    assert_eq!(*format, ImageFormat::Png);
    assert!(
        data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "data does not start with the PNG signature"
    );
}

#[test]
fn clips_and_layers_are_balanced() {
    for name in FIXTURES {
        let bg = load(name);
        let (mut clips, mut layers) = (0i32, 0i32);
        for (i, op) in bg.ops.iter().enumerate() {
            match op {
                DrawOp::PushClip { .. } => clips += 1,
                DrawOp::PopClip => clips -= 1,
                DrawOp::PushLayer { .. } => layers += 1,
                DrawOp::PopLayer => layers -= 1,
                _ => {}
            }
            assert!(
                clips >= 0 && layers >= 0,
                "{name}: unbalanced pop at op {i}"
            );
        }
        assert_eq!(
            (clips, layers),
            (0, 0),
            "{name}: unclosed clips {clips}, layers {layers}"
        );
    }
    assert!(
        load("clip")
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::PushClip { .. })),
        "clip.svg emitted no clip"
    );
}

#[test]
fn gradients_are_preserved() {
    let bg = load("gradient");
    let gradient = bg.ops.iter().find_map(|op| match op {
        DrawOp::FillPath {
            paint: Paint::LinearGradient { stops, .. },
            ..
        } => Some(stops.clone()),
        _ => None,
    });
    let stops = gradient.unwrap_or_else(|| panic!("no linear gradient: {:?}", bg.ops));
    let offsets: Vec<f32> = stops.iter().map(|s| s.offset).collect();
    assert_eq!(offsets, vec![0.0, 1.0], "stops {stops:?}");
}

#[test]
fn unsupported_features_warn_once() {
    let bg = load("unsupported");
    let expected = vec![
        BackgroundWarning::UnsupportedText,
        BackgroundWarning::UnsupportedFilter,
        BackgroundWarning::UnsupportedMask,
        BackgroundWarning::UnsupportedPattern,
    ];
    assert_eq!(bg.warnings, expected);
    let fills = solid_fills(&bg);
    assert!(
        fills.contains(&rgb(255, 0, 0)) && fills.contains(&rgb(0, 255, 0)),
        "filtered/masked content not drawn: {fills:?}"
    );
}

#[test]
fn external_hrefs_are_never_loaded() {
    let secret = std::fs::read(fixtures_dir().join("backgrounds/secret.png")).expect("secret.png");
    let bg = load("external-href");
    assert!(
        !bg.ops.iter().any(|op| matches!(op, DrawOp::Image { .. })),
        "an external image was loaded: {:?}",
        bg.ops
    );
    assert_eq!(
        bg.warnings,
        vec![BackgroundWarning::UnsupportedExternalImage]
    );
    let leaked = bg.ops.iter().any(|op| match op {
        DrawOp::Image { data, .. } => data.windows(secret.len()).any(|w| w == secret.as_slice()),
        _ => false,
    });
    assert!(!leaked, "secret.png bytes appear in the ops");
}

#[test]
fn invalid_svg_is_an_error() {
    let result = parse_background(&read("invalid"));
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "{:?}",
        result.map(|bg| bg.ops.len())
    );
}

#[test]
fn fit_transform_letterboxes() {
    assert_eq!(
        fit_transform(200.0, 100.0, 100.0, 100.0),
        Affine::scale_translate(0.5, 0.0, 25.0)
    );
}

/// Nests `<use>` ten-way `levels` deep, so the innermost element is drawn 10^levels times.
fn repeated(inner: &str, levels: usize) -> String {
    let mut defs = format!(r#"<g id="l0">{inner}</g>"#);
    for level in 1..=levels {
        let uses: String = (0..10)
            .map(|_| format!(r##"<use href="#l{}"/>"##, level - 1))
            .collect();
        defs.push_str(&format!(r#"<g id="l{level}">{uses}</g>"#));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>{defs}</defs><use href="#l{levels}"/></svg>"##
    )
}

#[test]
fn repeated_paths_are_capped() {
    let svg = repeated(r#"<rect width="1" height="1"/>"#, 5);
    let result = parse_background(&svg);
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "100000 repeated paths parsed: {:?}",
        result.map(|b| b.ops.len())
    );
}

#[test]
fn repeated_image_bytes_are_capped() {
    let png = std::fs::read(fixtures_dir().join("backgrounds/large.png")).expect("large.png");
    let encoded = base64_encode(&png);
    let svg = repeated(
        &format!(r#"<image width="1" height="1" href="data:image/png;base64,{encoded}"/>"#),
        3,
    );
    let result = parse_background(&svg);
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "1000 copies of a {} byte image parsed: {:?}",
        png.len(),
        result.map(|b| b.ops.len())
    );
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &b)| acc | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                TABLE[(n >> (18 - 6 * i)) as usize & 63] as char
            } else {
                '='
            });
        }
    }
    out
}

#[test]
fn repeated_path_geometry_is_capped() {
    let d: String = (0..50_000)
        .map(|i| format!("L{} {} ", i % 7, i % 5))
        .collect();
    let svg = repeated(
        &format!(r#"<path d="M0 0 {d}" fill="none" stroke="black"/>"#),
        2,
    );
    let result = parse_background(&svg);
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "100 copies of a 50,000-segment path parsed: {:?}",
        result.map(|b| b.ops.len())
    );
}

#[test]
fn huge_image_dimensions_are_capped() {
    let png = std::fs::read(fixtures_dir().join("backgrounds/huge-dimensions.png"))
        .expect("huge-dimensions.png");
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image width="10" height="10" href="data:image/png;base64,{}"/></svg>"#,
        base64_encode(&png)
    );
    let result = parse_background(&svg);
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "a {} byte PNG declaring 20000×20000 pixels parsed: {:?}",
        png.len(),
        result.map(|b| b.ops.len())
    );
}

fn repeated_nested_svg(inner_levels: usize, outer_levels: usize) -> String {
    let inner = repeated(r#"<rect width="1" height="1"/>"#, inner_levels);
    repeated(
        &format!(
            r#"<image width="1" height="1" href="data:image/svg+xml;base64,{}"/>"#,
            base64_encode(inner.as_bytes())
        ),
        outer_levels,
    )
}

fn assert_rejected_quickly(svg: &str, limit: std::time::Duration, what: &str) {
    let started = std::time::Instant::now();
    let result = parse_background(svg);
    let elapsed = started.elapsed();
    assert!(
        matches!(result, Err(BackgroundError::Invalid { .. })),
        "{what} parsed: {:?}",
        result.map(|b| b.ops.len())
    );
    assert!(
        elapsed < limit,
        "rejecting {what} ({} input bytes) took {elapsed:?}; nested SVG parses must share one budget",
        svg.len()
    );
}

#[test]
fn repeated_nested_svg_images_are_capped() {
    assert_rejected_quickly(
        &repeated_nested_svg(4, 3),
        std::time::Duration::from_secs(2),
        "1000 copies of a 10,000-node nested SVG",
    );
}

#[test]
fn repeated_unparseable_nested_svg_is_parsed_once() {
    let svg = repeated_nested_svg(6, 2);
    let started = std::time::Instant::now();
    let result = parse_background(&svg);
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "100 uses of a nested SVG past usvg's element limit took {elapsed:?}: {:?}",
        result.map(|b| b.ops.len())
    );
}

/// A nested SVG whose expanded `<use>` content sits in a clip path, outside the drawn tree.
fn clip_heavy_nested_svg(salt: usize) -> String {
    let mut defs = format!(r#"<g id="c0"><g><desc>{salt}</desc></g></g>"#);
    for level in 1..=4 {
        let uses: String = (0..10)
            .map(|_| format!(r##"<use href="#c{}"/>"##, level - 1))
            .collect();
        defs.push_str(&format!(r#"<g id="c{level}">{uses}</g>"#));
    }
    let inner = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>{defs}<clipPath id="clip"><use href="#c4"/></clipPath></defs><rect width="10" height="10" clip-path="url(#clip)"/></svg>"##
    );
    format!(
        r#"<image width="1" height="1" href="data:image/svg+xml;base64,{}"/>"#,
        base64_encode(inner.as_bytes())
    )
}

#[test]
fn many_distinct_nested_svg_images_are_capped() {
    let images: String = (0..20).map(clip_heavy_nested_svg).collect();
    assert_rejected_quickly(
        &repeated(&images, 1),
        std::time::Duration::from_secs(5),
        "200 uses of 20 distinct nested SVGs whose content hides in clip paths",
    );
}
