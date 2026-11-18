use sketch_core::{
    outline_stroke, Affine, Color, Document, InputSample, LiveUpdate, PathData, Pen, PointerKind,
    Session, Stroke, TAIL_MAX_VERBS,
};

const FRAME_MS: f64 = 16.0;

fn pen(min: f64, max: f64) -> Pen {
    Pen {
        color: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        min_width: min,
        max_width: max,
    }
}

fn session() -> Session {
    Session::new(Document::new(1000.0, 1000.0).expect("valid size"))
}

fn touch(x: f64, y: f64, time_ms: f64) -> InputSample {
    InputSample {
        x,
        y,
        time_ms,
        pressure: None,
        kind: PointerKind::Touch,
    }
}

fn stylus(x: f64, y: f64, time_ms: f64, pressure: f64) -> InputSample {
    InputSample {
        x,
        y,
        time_ms,
        pressure: Some(pressure),
        kind: PointerKind::Stylus,
    }
}

/// Draws `samples` as one stroke (first sample begins it) and returns the committed stroke.
fn draw(session: &mut Session, pen: Pen, view_to_doc: Affine, samples: &[InputSample]) -> Stroke {
    session.begin_stroke(pen, view_to_doc, samples[0]);
    session.extend_stroke(&samples[1..], &[]);
    session.end_stroke().expect("live stroke");
    session
        .document()
        .strokes
        .last()
        .expect("committed stroke")
        .clone()
}

fn horizontal(count: usize, step_pt: f64) -> Vec<InputSample> {
    (0..count)
        .map(|i| touch(10.0 + i as f64 * step_pt, 100.0, i as f64 * FRAME_MS))
        .collect()
}

fn widths(stroke: &Stroke) -> Vec<f64> {
    stroke.points.iter().map(|p| p.w).collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Deterministic pseudo-random sequence in [0, 1).
fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn fixed_pen_gives_exact_widths() {
    let stroke = draw(
        &mut session(),
        pen(2.0, 2.0),
        Affine::scale_translate(0.5, 0.0, 0.0),
        &horizontal(30, 5.0),
    );
    assert!(
        stroke.points.iter().all(|p| p.w == 4.0),
        "widths: {:?}",
        widths(&stroke)
    );
}

#[test]
fn view_points_map_through_transform() {
    let stroke = draw(
        &mut session(),
        pen(1.0, 3.0),
        Affine::scale_translate(2.0, -10.0, 0.0),
        &[touch(30.0, 40.0, 0.0)],
    );
    let p = stroke.points[0];
    assert_eq!((p.x, p.y), (50.0, 80.0), "stored point {p:?}");
}

#[test]
fn slow_touch_is_thicker_than_fast() {
    let slow = draw(
        &mut session(),
        pen(1.0, 3.0),
        Affine::IDENTITY,
        &horizontal(200, 1.0),
    );
    let fast = draw(
        &mut session(),
        pen(1.0, 3.0),
        Affine::IDENTITY,
        &horizontal(40, 20.0),
    );
    let (slow_mean, fast_mean) = (mean(&widths(&slow)), mean(&widths(&fast)));
    assert!(
        slow_mean > fast_mean + 1.0,
        "slow mean {slow_mean:.3} should exceed fast mean {fast_mean:.3} by > 1.0\nslow: {:?}\nfast: {:?}",
        widths(&slow),
        widths(&fast)
    );
}

#[test]
fn widths_stay_within_bounds() {
    let (min, max, scale) = (1.0, 6.0, 0.25);
    let mut seed = 7;
    for kind in [PointerKind::Touch, PointerKind::Stylus] {
        let (mut x, mut t) = (0.0, 0.0);
        let samples: Vec<InputSample> = (0..400)
            .map(|_| {
                x += lcg(&mut seed) * 40.0;
                t += 1.0 + lcg(&mut seed) * 30.0;
                let pressure = (kind == PointerKind::Stylus).then(|| lcg(&mut seed) * 1.4 - 0.2);
                InputSample {
                    x,
                    y: 50.0,
                    time_ms: t,
                    pressure,
                    kind,
                }
            })
            .collect();
        let stroke = draw(
            &mut session(),
            pen(min, max),
            Affine::scale_translate(scale, 0.0, 0.0),
            &samples,
        );
        for (i, w) in widths(&stroke).iter().enumerate() {
            assert!(
                (min / scale..=max / scale).contains(w),
                "{kind:?} point {i}: width {w} outside [{}, {}]",
                min / scale,
                max / scale
            );
        }
    }
}

#[test]
fn width_eases_on_speed_step() {
    let (min, max) = (1.0, 3.0);
    let mut samples = horizontal(60, 1.0);
    let (last_x, last_t) = (samples[59].x, samples[59].time_ms);
    samples.extend((1..40).map(|i| {
        touch(
            last_x + i as f64 * 20.0,
            100.0,
            last_t + i as f64 * FRAME_MS,
        )
    }));
    let stroke = draw(&mut session(), pen(min, max), Affine::IDENTITY, &samples);
    let w = widths(&stroke);
    for (i, pair) in w.windows(2).enumerate() {
        let step = (pair[1] - pair[0]).abs();
        assert!(
            step <= 0.25 * (max - min),
            "width stepped {step:.3} between points {i} and {}: {w:?}",
            i + 1
        );
    }
}

#[test]
fn stylus_pressure_drives_width() {
    let (min, max) = (1.0, 5.0);
    for (pressure, expected) in [(1.0, max), (0.0, min)] {
        let samples: Vec<InputSample> = (0..50)
            .map(|i| stylus(10.0 + i as f64 * 4.0, 10.0, i as f64 * FRAME_MS, pressure))
            .collect();
        let stroke = draw(&mut session(), pen(min, max), Affine::IDENTITY, &samples);
        let last = stroke.points.last().expect("points").w;
        assert!(
            (last - expected).abs() <= 0.02 * expected,
            "pressure {pressure}: last width {last}, expected {expected} ±2%; widths {:?}",
            widths(&stroke)
        );
    }
}

#[test]
fn decimation_drops_dense_samples() {
    let samples = horizontal(100, 0.1);
    let stroke = draw(&mut session(), pen(1.0, 3.0), Affine::IDENTITY, &samples);
    let last = stroke.points.last().expect("points");
    assert!(
        stroke.points.len() < 100,
        "stored {} of 100 dense samples",
        stroke.points.len()
    );
    assert_eq!(
        (last.x, last.y),
        (samples[99].x, samples[99].y),
        "final sample not stored: {:?}",
        stroke.points
    );
}

#[test]
fn tap_is_one_point() {
    let stroke = draw(
        &mut session(),
        pen(1.0, 3.0),
        Affine::IDENTITY,
        &[touch(5.0, 5.0, 0.0)],
    );
    assert_eq!(stroke.points.len(), 1, "{stroke:?}");
}

#[test]
fn predicted_samples_are_not_stored() {
    let samples = horizontal(20, 5.0);
    let plain = draw(&mut session(), pen(1.0, 3.0), Affine::IDENTITY, &samples);
    let mut s = session();
    s.begin_stroke(pen(1.0, 3.0), Affine::IDENTITY, samples[0]);
    for (i, sample) in samples[1..].iter().enumerate() {
        let predicted = [
            touch(sample.x + 7.0, 140.0, sample.time_ms + 8.0),
            touch(sample.x + 14.0, 180.0, sample.time_ms + 16.0),
        ];
        let update = s.extend_stroke(std::slice::from_ref(sample), &predicted);
        assert!(
            !update.tail.is_empty(),
            "sample {i}: empty tail with predicted samples"
        );
    }
    s.end_stroke().expect("live stroke");
    let predicted = s.document().strokes.last().expect("stroke").clone();
    assert_eq!(
        predicted, plain,
        "predicted samples changed the stored stroke"
    );
}

#[test]
fn long_stroke_updates_are_bounded() {
    let samples: Vec<InputSample> = (0..5000)
        .map(|i| {
            let a = i as f64 * 0.01;
            touch(
                500.0 + (100.0 + a * 10.0) * a.cos(),
                500.0 + (100.0 + a * 10.0) * a.sin(),
                i as f64 * 4.0,
            )
        })
        .collect();
    let mut s = session();
    let mut settled = PathData::default();
    let mut last_tail = PathData::default();
    let mut record = |update: LiveUpdate, label: &str| {
        assert!(
            update.tail.verbs.len() <= TAIL_MAX_VERBS,
            "{label}: tail has {} verbs (max {TAIL_MAX_VERBS})",
            update.tail.verbs.len()
        );
        settled.append(&update.settled);
        last_tail = update.tail;
    };
    record(
        s.begin_stroke(pen(1.0, 3.0), Affine::IDENTITY, samples[0]),
        "begin",
    );
    for (i, chunk) in samples[1..].chunks(4).enumerate() {
        record(s.extend_stroke(chunk, &[]), &format!("chunk {i}"));
    }
    s.end_stroke().expect("live stroke");
    let whole = outline_stroke(s.document().strokes.last().expect("stroke"));
    assert!(
        whole.verbs.starts_with(&settled.verbs),
        "settled verbs are not a prefix of the committed outline"
    );
    assert!(
        whole.coords.starts_with(&settled.coords),
        "settled coords are not a prefix of the committed outline"
    );
    settled.append(&last_tail);
    assert_eq!(
        settled, whole,
        "settled + final tail differs from the committed outline"
    );
}
