//! Turns pointer samples into stored stroke points. These constants are capture tuning, not
//! part of the document format: changing them only affects strokes drawn afterwards.

use crate::{outline_segments, outline_tail, Affine, Color, PathData, Point, Stroke};

/// A live update's tail never exceeds this many verbs; a longer one is a bug.
pub const TAIL_MAX_VERBS: usize = 512;

/// View points between stored samples.
const MIN_SAMPLE_DISTANCE: f64 = 1.5;
/// Touch speed (view points per millisecond) at which the pen reaches its minimum width.
const REFERENCE_SPEED: f64 = 1.5;
/// Exponential moving average weights given to each new measurement.
const SPEED_SMOOTHING: f64 = 0.25;
const PRESSURE_SMOOTHING: f64 = 0.4;
const MIN_PEN_WIDTH: f64 = 0.01;

#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Touch,
    Stylus,
}

/// One pointer sample in view points.
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputSample {
    pub x: f64,
    pub y: f64,
    pub time_ms: f64,
    pub pressure: Option<f64>,
    pub kind: PointerKind,
}

/// Widths are in view points.
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pen {
    pub color: Color,
    pub min_width: f64,
    pub max_width: f64,
}

/// Outline geometry in document units. `settled` extends what earlier updates sent;
/// `tail` replaces the previous tail.
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveUpdate {
    pub settled: PathData,
    pub tail: PathData,
}

#[derive(Debug, Clone, Copy)]
struct WidthModel {
    min: f64,
    max: f64,
    speed: f64,
    pressure: Option<f64>,
    last: InputSample,
}

impl WidthModel {
    fn start(pen: &Pen, sample: InputSample) -> (WidthModel, f64) {
        let min = pen.min_width.min(pen.max_width).max(MIN_PEN_WIDTH);
        let max = pen.min_width.max(pen.max_width).max(MIN_PEN_WIDTH);
        let mut model = WidthModel {
            min,
            max,
            speed: REFERENCE_SPEED / 2.0,
            pressure: None,
            last: sample,
        };
        let width = model.width_for(sample, None);
        (model, width)
    }

    fn advance(&mut self, sample: InputSample) -> f64 {
        let elapsed = sample.time_ms - self.last.time_ms;
        let speed = (elapsed > 0.0)
            .then(|| (sample.x - self.last.x).hypot(sample.y - self.last.y) / elapsed);
        self.last = sample;
        self.width_for(sample, speed)
    }

    fn width_for(&mut self, sample: InputSample, measured_speed: Option<f64>) -> f64 {
        if self.min == self.max {
            return self.max;
        }
        match (sample.kind, sample.pressure) {
            (PointerKind::Stylus, Some(raw)) => {
                let pressure = raw.clamp(0.0, 1.0);
                let smoothed = self
                    .pressure
                    .map_or(pressure, |p| p + PRESSURE_SMOOTHING * (pressure - p));
                self.pressure = Some(smoothed);
                self.min + (self.max - self.min) * smoothed
            }
            _ => {
                if let Some(speed) = measured_speed {
                    self.speed += SPEED_SMOOTHING * (speed - self.speed);
                }
                self.max - (self.max - self.min) * (self.speed / REFERENCE_SPEED).clamp(0.0, 1.0)
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct StrokeBuilder {
    color: Color,
    view_to_doc: Affine,
    scale: f64,
    model: WidthModel,
    points: Vec<Point>,
    last_stored: InputSample,
    pending: Option<InputSample>,
    settled_next: usize,
}

impl StrokeBuilder {
    pub(crate) fn begin(pen: Pen, view_to_doc: Affine, sample: InputSample) -> StrokeBuilder {
        let (model, width) = WidthModel::start(&pen, sample);
        let scale = view_to_doc.a.hypot(view_to_doc.b);
        let mut builder = StrokeBuilder {
            color: pen.color,
            view_to_doc,
            scale,
            model,
            points: Vec::new(),
            last_stored: sample,
            pending: None,
            settled_next: 0,
        };
        let point = builder.to_document(sample, width);
        builder.points.push(point);
        builder
    }

    pub(crate) fn extend(&mut self, samples: &[InputSample]) {
        for &sample in samples {
            let distance = (sample.x - self.last_stored.x).hypot(sample.y - self.last_stored.y);
            if distance < MIN_SAMPLE_DISTANCE {
                self.pending = Some(sample);
            } else {
                self.store(sample);
            }
        }
    }

    /// Settled geometry since the last call, plus a tail through the latest and predicted samples.
    pub(crate) fn live_update(&mut self, predicted: &[InputSample]) -> LiveUpdate {
        let (settled, next) = outline_segments(&self.points, self.settled_next);
        self.settled_next = next;
        let mut provisional = self.points.clone();
        let mut model = self.model;
        for &sample in self.pending.iter().chain(predicted) {
            let width = model.advance(sample);
            provisional.push(self.to_document(sample, width));
        }
        LiveUpdate {
            settled,
            tail: outline_tail(&provisional, self.settled_next),
        }
    }

    pub(crate) fn finish(mut self) -> Stroke {
        if let Some(sample) = self.pending.take() {
            if (sample.x, sample.y) != (self.last_stored.x, self.last_stored.y) {
                self.store(sample);
            }
        }
        Stroke {
            color: self.color,
            points: self.points,
        }
    }

    fn store(&mut self, sample: InputSample) {
        let width = self.model.advance(sample);
        let point = self.to_document(sample, width);
        self.points.push(point);
        self.last_stored = sample;
        self.pending = None;
    }

    fn to_document(&self, sample: InputSample, view_width: f64) -> Point {
        let (x, y) = self.view_to_doc.apply(sample.x, sample.y);
        Point {
            x,
            y,
            w: view_width / self.scale,
        }
    }
}
