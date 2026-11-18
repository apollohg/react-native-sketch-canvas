//! The frozen v1 stroke outline algorithm. See FORMAT.md; any change here is a new format version.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::math::{acos, atan2, cos, hypot, sin, tan};
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
    let stable_len = build_centre_line(points, &mut samples);
    (samples, stable_len)
}

/// The number of samples in a stroke's centre line, without building it.
pub(crate) fn centre_line_len(points: &[Point]) -> usize {
    let mut count = SampleCount::default();
    build_centre_line(points, &mut count);
    count.len
}

/// Where centre line samples go; only the last one is needed to drop duplicates.
trait Samples {
    fn last(&self) -> Option<Point>;
    fn add(&mut self, sample: Point);
    fn len(&self) -> usize;
}

impl Samples for Vec<Point> {
    fn last(&self) -> Option<Point> {
        self.as_slice().last().copied()
    }
    fn add(&mut self, sample: Point) {
        self.push(sample);
    }
    fn len(&self) -> usize {
        self.as_slice().len()
    }
}

#[derive(Default)]
struct SampleCount {
    last: Option<Point>,
    len: usize,
}

impl Samples for SampleCount {
    fn last(&self) -> Option<Point> {
        self.last
    }
    fn add(&mut self, sample: Point) {
        self.last = Some(sample);
        self.len += 1;
    }
    fn len(&self) -> usize {
        self.len
    }
}

/// Builds S into `samples`; returns how many leading samples are stable.
fn build_centre_line(points: &[Point], samples: &mut impl Samples) -> usize {
    let Some(&last) = points.last() else {
        return 0;
    };
    if points.len() < 3 {
        for &p in points {
            push_sample(samples, p);
        }
        return samples.len().min(1);
    }
    push_sample(samples, points[0]);
    push_sample(samples, midpoint(points[0], points[1]));
    for window in points.windows(3) {
        push_quadratic(samples, window);
    }
    let stable_len = samples.len();
    push_sample(samples, last);
    stable_len
}

/// The samples of S that appending points can never change, built as a stroke grows.
#[derive(Debug, Default)]
pub(crate) struct StableCentreLine {
    samples: Vec<Point>,
    quadratics: usize,
}

impl StableCentreLine {
    /// Adds the quadratics completed since the last call; `points` only ever grows.
    pub(crate) fn extend(&mut self, points: &[Point]) {
        if points.len() < 3 {
            return;
        }
        if self.samples.is_empty() {
            push_sample(&mut self.samples, points[0]);
            push_sample(&mut self.samples, midpoint(points[0], points[1]));
        }
        for window in points[self.quadratics..].windows(3) {
            push_quadratic(&mut self.samples, window);
        }
        self.quadratics = points.len() - 2;
    }

    pub(crate) fn samples(&self) -> &[Point] {
        &self.samples
    }
}

fn push_quadratic(samples: &mut impl Samples, window: &[Point]) {
    let (start, control, end) = (
        midpoint(window[0], window[1]),
        window[1],
        midpoint(window[1], window[2]),
    );
    let max_radius = (start.w / 2.0).max(control.w / 2.0).max(end.w / 2.0);
    let tolerance = (FLATTEN_TOLERANCE_PER_RADIUS * max_radius).max(MIN_FLATTEN_TOLERANCE);
    let second_difference = hypot(
        start.x - 2.0 * control.x + end.x,
        start.y - 2.0 * control.y + end.y,
    );
    let steps = ((second_difference / (4.0 * tolerance)).sqrt().ceil() as usize)
        .clamp(1, MAX_FLATTEN_STEPS);
    for j in 1..=steps {
        let t = j as f64 / steps as f64;
        let quad = |a: f64, b: f64, c: f64| {
            (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * b + t * t * c
        };
        push_sample(
            samples,
            Point {
                x: quad(start.x, control.x, end.x),
                y: quad(start.y, control.y, end.y),
                w: quad(start.w, control.w, end.w),
            },
        );
    }
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
        w: (a.w + b.w) / 2.0,
    }
}

fn push_sample(samples: &mut impl Samples, p: Point) {
    if let Some(last) = samples.last() {
        if hypot(p.x - last.x, p.y - last.y) <= DUPLICATE_SAMPLE_DISTANCE {
            return;
        }
    }
    samples.add(p);
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
    (
        pieces_between(&samples, from_sample, settled_end),
        settled_end,
    )
}

/// The pieces for sample pairs `from..to`.
pub(crate) fn pieces_between(samples: &[Point], from: usize, to: usize) -> PathData {
    match samples.get(from..=to) {
        Some(range) if to > from => pieces(range, false),
        _ => PathData::default(),
    }
}

/// Emits every piece from sample pair `from_sample` to the end of the stroke.
pub fn outline_tail(points: &[Point], from_sample: usize) -> PathData {
    let samples = centre_line(points);
    pieces(
        samples.get(from_sample..).unwrap_or_default(),
        from_sample == 0,
    )
}

/// The pieces after a stroke's stable samples, without revisiting earlier points.
/// `stable_from` is the stable samples from sample `from_sample` on; `recent` is the stroke's
/// last two points followed by any provisional ones.
pub(crate) fn live_tail(stable_from: &[Point], recent: &[Point], from_sample: usize) -> PathData {
    let mut samples = stable_from.to_vec();
    for window in recent.windows(3) {
        push_quadratic(&mut samples, window);
    }
    if let Some(&last) = recent.last() {
        push_sample(&mut samples, last);
    }
    pieces(&samples, from_sample == 0)
}

/// The pieces for consecutive samples; a lone sample of a whole stroke is a circle.
fn pieces(samples: &[Point], whole: bool) -> PathData {
    let mut path = PathData::default();
    if let ([sample], true) = (samples, whole) {
        emit_circle(&mut path, *sample);
    }
    for pair in samples.windows(2) {
        emit_pair(&mut path, pair[0], pair[1]);
    }
    path
}

fn emit_pair(path: &mut PathData, a: Point, b: Point) {
    let (ra, rb) = (a.w / 2.0, b.w / 2.0);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let distance = hypot(dx, dy);
    if distance <= (ra - rb).abs() {
        emit_circle(path, if ra >= rb { a } else { b });
        return;
    }
    let theta = atan2(dy, dx);
    let beta = acos((ra - rb) / distance);
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
    (centre.x + r * cos(angle), centre.y + r * sin(angle))
}

fn emit_arc(path: &mut PathData, centre: Point, r: f64, start: f64, end: f64) {
    let delta = end - start;
    let parts = ((delta.abs() / FRAC_PI_2).ceil() as usize).max(1);
    let step = delta / parts as f64;
    let handle = r * 4.0 / 3.0 * tan(step / 4.0);
    for i in 0..parts {
        let a0 = start + step * i as f64;
        let a1 = a0 + step;
        let (x0, y0) = polar(centre, r, a0);
        let (x3, y3) = polar(centre, r, a1);
        path.cubic_to(
            x0 - handle * sin(a0),
            y0 + handle * cos(a0),
            x3 + handle * sin(a1),
            y3 - handle * cos(a1),
            x3,
            y3,
        );
    }
}
