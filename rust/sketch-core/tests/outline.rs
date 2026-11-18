mod common;

use common::{fixtures_dir, load_fixture_document, rasterize, shoelace_area, subpaths};
use sketch_core::{
    centre_line, outline_segments, outline_stroke, outline_tail, Color, PathData, Point, Stroke,
};

const BLACK: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};
const FULL: f32 = 0.99;
const AREA_SUPERSAMPLE: f32 = 8.0;

fn stroke(points: &[(f64, f64, f64)]) -> Stroke {
    Stroke {
        color: BLACK,
        points: points.iter().map(|&(x, y, w)| Point { x, y, w }).collect(),
    }
}

fn fixture_stroke(name: &str) -> Stroke {
    load_fixture_document(&format!("conformance/{name}.json"))
        .strokes
        .remove(0)
}

fn distance_to_polyline(x: f64, y: f64, line: &[Point]) -> f64 {
    if line.len() == 1 {
        return (x - line[0].x).hypot(y - line[0].y);
    }
    line.windows(2)
        .map(|seg| {
            let (ax, ay, bx, by) = (seg[0].x, seg[0].y, seg[1].x, seg[1].y);
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            let t = if len2 == 0.0 {
                0.0
            } else {
                (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0)
            };
            (x - ax - t * dx).hypot(y - ay - t * dy)
        })
        .fold(f64::MAX, f64::min)
}

/// Rasterises at `AREA_SUPERSAMPLE`× so edge anti-aliasing bias stays well under the tolerance.
fn assert_area(name: &str, data: &PathData, expected: f64) {
    let scaled = PathData {
        verbs: data.verbs.clone(),
        coords: data.coords.iter().map(|c| c * AREA_SUPERSAMPLE).collect(),
    };
    let area =
        rasterize(&scaled, 4.0).total_coverage() / f64::from(AREA_SUPERSAMPLE * AREA_SUPERSAMPLE);
    let error = (area - expected).abs() / expected;
    assert!(
        error <= 0.005,
        "{name}: raster area {area:.3}, expected {expected:.3} (relative error {error:.5})"
    );
}

#[test]
fn single_point_is_a_disc() {
    let path = outline_stroke(&stroke(&[(50.0, 50.0, 20.0)]));
    let xs = path.coords.iter().step_by(2).copied();
    let ys = path.coords.iter().skip(1).step_by(2).copied();
    let (min_x, max_x) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (min_y, max_y) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    for (label, actual, expected) in [
        ("min x", min_x, 40.0),
        ("max x", max_x, 60.0),
        ("min y", min_y, 40.0),
        ("max y", max_y, 60.0),
    ] {
        assert!(
            (actual - expected).abs() <= 1e-4,
            "{label}: {actual} != {expected}; path {path:?}"
        );
    }
    assert_area("dot", &path, std::f64::consts::PI * 100.0);
}

#[test]
fn straight_constant_width_is_capsule() {
    let path = outline_stroke(&stroke(&[(10.0, 50.0, 8.0), (90.0, 50.0, 8.0)]));
    assert_area(
        "capsule",
        &path,
        2.0 * 4.0 * 80.0 + std::f64::consts::PI * 16.0,
    );
}

#[test]
fn coverage_matches_distance_field() {
    for name in ["hairpin", "zigzag", "spiral"] {
        let stroke = fixture_stroke(name);
        let r = stroke.points[0].w / 2.0;
        let line = centre_line(&stroke.points);
        let raster = rasterize(&outline_stroke(&stroke), 4.0);
        for (x, y, coverage) in raster.pixels() {
            let d = distance_to_polyline(f64::from(x), f64::from(y), &line);
            if d <= r - 1.0 {
                assert!(coverage >= FULL, "{name}: pixel ({x}, {y}) at distance {d:.3} has coverage {coverage}, expected ≥ {FULL}");
            }
            if d > r + 1.0 {
                assert!(coverage == 0.0, "{name}: pixel ({x}, {y}) at distance {d:.3} has coverage {coverage}, expected 0");
            }
        }
    }
}

#[test]
fn variable_width_stays_within_bounds() {
    let stroke = fixture_stroke("taper");
    let line = centre_line(&stroke.points);
    let max_r = line.iter().map(|p| p.w / 2.0).fold(0.0, f64::max);
    let raster = rasterize(&outline_stroke(&stroke), 4.0);
    for (x, y, coverage) in raster.pixels() {
        let (x, y) = (f64::from(x), f64::from(y));
        let inside_a_disc = line
            .iter()
            .any(|p| (x - p.x).hypot(y - p.y) <= p.w / 2.0 - 1.0);
        if inside_a_disc {
            assert!(
                coverage >= FULL,
                "taper: pixel ({x}, {y}) is inside a sample disc but has coverage {coverage}"
            );
        }
        let d = distance_to_polyline(x, y, &line);
        if d > max_r + 1.0 {
            assert!(coverage == 0.0, "taper: pixel ({x}, {y}) at distance {d:.3} > max r {max_r} + 1 has coverage {coverage}");
        }
    }
}

/// The smoothed centre line turns at x ≈ 75 (the quadratic's apex), so the turn's inner region
/// must be solid where the two overlapping sides meet.
#[test]
fn hairpin_has_no_hole() {
    let hairpin = stroke(&[(0.0, 0.0, 10.0), (100.0, 0.0, 10.0), (0.0, 0.5, 10.0)]);
    let tip_x = centre_line(&hairpin.points)
        .iter()
        .map(|p| p.x)
        .fold(f64::MIN, f64::max);
    let raster = rasterize(&outline_stroke(&hairpin), 4.0);
    for x in [tip_x - 4.0, tip_x - 2.0, tip_x, tip_x + 2.0] {
        let coverage = raster.coverage_at_doc(x as f32, 0.0);
        assert!(
            coverage >= FULL,
            "hairpin pixel ({x:.2}, 0) near the turn (tip x {tip_x:.2}) has coverage {coverage}"
        );
    }
}

#[test]
fn every_subpath_has_negative_area() {
    for name in [
        "dot",
        "line",
        "taper",
        "hairpin",
        "zigzag",
        "spiral",
        "contained",
        "alpha-color",
    ] {
        for (index, stroke) in load_fixture_document(&format!("conformance/{name}.json"))
            .strokes
            .iter()
            .enumerate()
        {
            for (sub_index, sub) in subpaths(&outline_stroke(stroke)).iter().enumerate() {
                let area = shoelace_area(sub);
                assert!(area < 0.0, "{name} stroke {index} subpath {sub_index}: shoelace area {area} is not negative");
            }
        }
    }
}

#[test]
fn contained_disc_emits_one_circle() {
    let path = outline_stroke(&stroke(&[(0.0, 0.0, 40.0), (1.0, 0.0, 2.0)]));
    let count = subpaths(&path).len();
    assert_eq!(
        count, 1,
        "expected one circle, got {count} subpaths: {path:?}"
    );
}

#[test]
fn incremental_equals_whole() {
    for name in ["taper", "zigzag", "spiral", "hairpin"] {
        let stroke = fixture_stroke(name);
        let mut settled = PathData::default();
        let mut next = 0;
        for len in 1..=stroke.points.len() {
            let (segment, settled_end) = outline_segments(&stroke.points[..len], next);
            assert!(
                settled_end >= next,
                "{name}: settled index went backwards ({next} -> {settled_end}) at {len} points"
            );
            settled.append(&segment);
            next = settled_end;
        }
        settled.append(&outline_tail(&stroke.points, next));
        let whole = outline_stroke(&stroke);
        assert_eq!(
            settled.verbs, whole.verbs,
            "{name}: incremental verbs differ from whole stroke"
        );
        assert_eq!(
            settled.coords, whole.coords,
            "{name}: incremental coords differ from whole stroke"
        );
    }
}

#[test]
fn conformance_fixtures_match() {
    let dir = fixtures_dir().join("conformance");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("conformance dir") {
        let path = entry.expect("dir entry").path();
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !file.ends_with(".json") || file.ends_with(".outline.json") {
            continue;
        }
        let name = file.trim_end_matches(".json");
        let doc = load_fixture_document(&format!("conformance/{file}"));
        let expected_path = dir.join(format!("{name}.outline.json"));
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display())),
        )
        .expect("outline json");
        let expected_strokes = expected["strokes"].as_array().expect("strokes array");
        assert_eq!(
            expected_strokes.len(),
            doc.strokes.len(),
            "{name}: stroke count"
        );
        for (index, (stroke, exp)) in doc.strokes.iter().zip(expected_strokes).enumerate() {
            let actual = outline_stroke(stroke);
            let verbs: Vec<u8> = exp["verbs"]
                .as_array()
                .expect("verbs")
                .iter()
                .map(|v| v.as_u64().expect("verb") as u8)
                .collect();
            let coords: Vec<f64> = exp["coords"]
                .as_array()
                .expect("coords")
                .iter()
                .map(|v| v.as_f64().expect("coord"))
                .collect();
            assert_eq!(actual.verbs, verbs, "{name} stroke {index}: verbs differ");
            assert_eq!(
                actual.coords.len(),
                coords.len(),
                "{name} stroke {index}: coord count"
            );
            for (i, (a, e)) in actual.coords.iter().zip(&coords).enumerate() {
                assert!(
                    (f64::from(*a) - e).abs() <= 1e-3,
                    "{name} stroke {index} coord {i}: actual {a}, expected {e}"
                );
            }
        }
        checked += 1;
    }
    assert_eq!(
        checked,
        8,
        "expected 8 conformance fixtures in {}",
        dir.display()
    );
}

#[test]
fn hostile_coordinates_stay_bounded() {
    let hostile = stroke(&[
        (0.0, 0.0, 0.02),
        (1e12, 0.0, 0.02),
        (0.0, 1e12, 0.02),
        (1e300, -1e300, 0.02),
    ]);
    let samples = centre_line(&hostile.points).len();
    assert!(
        samples <= 3 + 2 * 64,
        "centre line has {samples} samples for 4 points"
    );
    let verbs = outline_stroke(&hostile).verbs.len();
    assert!(
        verbs <= 20 * samples,
        "outline has {verbs} verbs for {samples} samples"
    );
}
