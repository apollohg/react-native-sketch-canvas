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
