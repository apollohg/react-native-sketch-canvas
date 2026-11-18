//! The frozen v1 stroke outline algorithm. See FORMAT.md; any change here is a new format version.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::{Point, Stroke};

pub const VERB_MOVE: u8 = 0;
pub const VERB_LINE: u8 = 1;
pub const VERB_QUAD: u8 = 2;
pub const VERB_CUBIC: u8 = 3;
pub const VERB_CLOSE: u8 = 4;

const DUPLICATE_SAMPLE_DISTANCE: f64 = 1e-9;
const FLATTEN_TOLERANCE_PER_RADIUS: f64 = 0.02;
const MIN_FLATTEN_TOLERANCE: f64 = 0.01;
const MAX_FLATTEN_STEPS: usize = 64;

#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathData {
    pub verbs: Vec<u8>,
    pub coords: Vec<f32>,
}

impl PathData {
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn append(&mut self, other: &PathData) {
        self.verbs.extend_from_slice(&other.verbs);
        self.coords.extend_from_slice(&other.coords);
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        self.push(VERB_MOVE, &[x, y]);
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        self.push(VERB_LINE, &[x, y]);
    }

    pub fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64) {
        self.push(VERB_QUAD, &[x1, y1, x, y]);
    }

    pub fn cubic_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.push(VERB_CUBIC, &[x1, y1, x2, y2, x, y]);
    }

    pub fn close(&mut self) {
        self.verbs.push(VERB_CLOSE);
    }

    fn push(&mut self, verb: u8, coords: &[f64]) {
        self.verbs.push(verb);
        self.coords.extend(coords.iter().map(|&c| c as f32));
    }
}

/// The flattened centre line S of a stroke (step 2), with `w` holding the full width at each sample.
pub fn centre_line(points: &[Point]) -> Vec<Point> {
    centre_line_with_stable_len(points).0
}

/// Returns S plus the number of leading samples that appending further points can never change.
fn centre_line_with_stable_len(points: &[Point]) -> (Vec<Point>, usize) {
    let mut samples = Vec::new();
    let n = points.len();
    if n < 3 {
        for &p in points {
            push_sample(&mut samples, p);
        }
        let stable_len = samples.len().min(1);
        return (samples, stable_len);
    }
    push_sample(&mut samples, points[0]);
    push_sample(&mut samples, midpoint(points[0], points[1]));
    for window in points.windows(3) {
        let (start, control, end) = (
            midpoint(window[0], window[1]),
            window[1],
            midpoint(window[1], window[2]),
        );
        let max_radius = (start.w / 2.0).max(control.w / 2.0).max(end.w / 2.0);
        let tolerance = (FLATTEN_TOLERANCE_PER_RADIUS * max_radius).max(MIN_FLATTEN_TOLERANCE);
        let second_difference =
            (start.x - 2.0 * control.x + end.x).hypot(start.y - 2.0 * control.y + end.y);
        let steps = ((second_difference / (4.0 * tolerance)).sqrt().ceil() as usize)
            .clamp(1, MAX_FLATTEN_STEPS);
        for j in 1..=steps {
            let t = j as f64 / steps as f64;
            let quad = |a: f64, b: f64, c: f64| {
                (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * b + t * t * c
            };
            push_sample(
                &mut samples,
                Point {
                    x: quad(start.x, control.x, end.x),
                    y: quad(start.y, control.y, end.y),
                    w: quad(start.w, control.w, end.w),
                },
            );
        }
    }
    let stable_len = samples.len();
    push_sample(&mut samples, points[n - 1]);
    (samples, stable_len)
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
        w: (a.w + b.w) / 2.0,
    }
}

fn push_sample(samples: &mut Vec<Point>, p: Point) {
    if let Some(last) = samples.last() {
        if (p.x - last.x).hypot(p.y - last.y) <= DUPLICATE_SAMPLE_DISTANCE {
            return;
        }
    }
    samples.push(p);
}

/// The full outline of a stroke, filled with the non-zero rule.
pub fn outline_stroke(stroke: &Stroke) -> PathData {
    outline_tail(&stroke.points, 0)
}

/// Emits the pieces for sample pairs `from_sample..` that are final: appending points to
/// `points` never changes them. Returns the index to resume from on the next call.
pub fn outline_segments(points: &[Point], from_sample: usize) -> (PathData, usize) {
    let (samples, stable_len) = centre_line_with_stable_len(points);
    let settled_end = stable_len.saturating_sub(1).max(from_sample);
    let mut path = PathData::default();
    for pair in samples.windows(2).take(settled_end).skip(from_sample) {
        emit_pair(&mut path, pair[0], pair[1]);
    }
    (path, settled_end)
}

/// Emits every piece from sample pair `from_sample` to the end of the stroke.
pub fn outline_tail(points: &[Point], from_sample: usize) -> PathData {
    let samples = centre_line(points);
    let mut path = PathData::default();
    if samples.len() == 1 && from_sample == 0 {
        emit_circle(&mut path, samples[0]);
    }
    for pair in samples.windows(2).skip(from_sample) {
        emit_pair(&mut path, pair[0], pair[1]);
    }
    path
}

fn emit_pair(path: &mut PathData, a: Point, b: Point) {
    let (ra, rb) = (a.w / 2.0, b.w / 2.0);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let distance = dx.hypot(dy);
    if distance <= (ra - rb).abs() {
        emit_circle(path, if ra >= rb { a } else { b });
        return;
    }
    let theta = dy.atan2(dx);
    let beta = ((ra - rb) / distance).acos();
    let (a1x, a1y) = polar(a, ra, theta + beta);
    let (b1x, b1y) = polar(b, rb, theta + beta);
    let (a2x, a2y) = polar(a, ra, theta - beta);
    path.move_to(a1x, a1y);
    path.line_to(b1x, b1y);
    emit_arc(path, b, rb, theta + beta, theta - beta);
    path.line_to(a2x, a2y);
    emit_arc(path, a, ra, theta - beta, theta + beta - TAU);
    path.close();
}

fn emit_circle(path: &mut PathData, centre: Point) {
    let r = centre.w / 2.0;
    path.move_to(centre.x + r, centre.y);
    emit_arc(path, centre, r, 0.0, -TAU);
    path.close();
}

fn polar(centre: Point, r: f64, angle: f64) -> (f64, f64) {
    (centre.x + r * angle.cos(), centre.y + r * angle.sin())
}

fn emit_arc(path: &mut PathData, centre: Point, r: f64, start: f64, end: f64) {
    let delta = end - start;
    let parts = ((delta.abs() / FRAC_PI_2).ceil() as usize).max(1);
    let step = delta / parts as f64;
    let handle = r * 4.0 / 3.0 * (step / 4.0).tan();
    for i in 0..parts {
        let a0 = start + step * i as f64;
        let a1 = a0 + step;
        let (x0, y0) = polar(centre, r, a0);
        let (x3, y3) = polar(centre, r, a1);
        path.cubic_to(
            x0 - handle * a0.sin(),
            y0 + handle * a0.cos(),
            x3 + handle * a1.sin(),
            y3 - handle * a1.cos(),
            x3,
            y3,
        );
    }
}
