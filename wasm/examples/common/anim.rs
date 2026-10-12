//! The page's motion as SMIL keyframes, shared by the README animations.
//! SMIL has no linear() easing, so the spring is written out as keyframes, and
//! sizes are animated as x/y/width/height rather than transforms.
#![allow(dead_code)]

/// The page's --motion: every page duration below is scaled by it.
pub const MOTION: f64 = 0.75;
/// The page's --spring (stiffness 170, damping 16), sampled.
pub const SPRING: [f64; 21] = [
    0.0, 0.103, 0.326, 0.572, 0.784, 0.938, 1.033, 1.077, 1.087, 1.076, 1.056, 1.034, 1.017, 1.004, 0.996,
    0.992, 0.993, 0.995, 0.997, 0.999, 1.0,
];

// The page's timings.
pub const DROP: f64 = 0.5 * MOTION; // a piece landing, a ghost springing in
pub const SETTLE: f64 = 0.45 * MOTION; // the hover frame and capture hints springing
pub const FADE: f64 = 0.3 * MOTION; // a captured piece fading
pub const RIPPLE_START: f64 = 0.06 * MOTION; // the first capture after the move
pub const RIPPLE_STEP: f64 = 0.07 * MOTION; // each step further from the move
pub const FRAME_FADE: f64 = 0.15 * MOTION; // the hover frame appearing or hiding
pub const PULSE: f64 = 0.4 * MOTION; // a winning corner's pulse
pub const PULSE_STEP: f64 = 0.09 * MOTION; // between corners, clockwise from the top left
pub const TRACE_DELAY: f64 = 0.36 * MOTION; // the win's frame starts after the corners
pub const TRACE: f64 = 0.6 * MOTION; // the win's frame drawing itself

pub const CELL: f64 = 44.0;
pub const PAD: f64 = 28.0;
pub const SQUARE: f64 = CELL - 4.0; // a square's drawn size, inside the grid gap

pub const EMPTY: &str = "#2a2a2a";
pub const GREEN: &str = "#4CAF50";
pub const RED: &str = "#F44336";
pub const ACCENT: &str = "#ff9900";

pub fn name(i: usize) -> String {
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}

pub fn centre(c: usize) -> (f64, f64) {
    (PAD + (c % 5) as f64 * CELL + CELL / 2.0, PAD + (c / 5) as f64 * CELL + CELL / 2.0)
}

/// SMIL keyTimes/values for one attribute, from (seconds, value) keyframes in
/// time order. Equal times are nudged apart (an instant change).
pub fn smil(attr: &str, frames: &[(f64, String)], total: f64) -> String {
    let mut times = Vec::new();
    let mut values = Vec::new();
    let mut prev = -1.0;
    let end = frames.last().unwrap().1.clone();
    for (t, v) in frames.iter().cloned().chain(std::iter::once((total, end))) {
        let mut k = (t / total).clamp(0.0, 1.0);
        if k <= prev {
            k = prev + 1e-4;
        }
        prev = k;
        times.push(format!("{:.4}", k.min(1.0)));
        values.push(v);
    }
    *times.last_mut().unwrap() = "1".to_string();
    format!(
        "<animate attributeName=\"{attr}\" dur=\"{total:.2}s\" repeatCount=\"indefinite\" keyTimes=\"{}\" values=\"{}\"/>",
        times.join(";"),
        values.join(";")
    )
}

/// One animated number as keyframes, held flat between changes.
pub struct Track(pub Vec<(f64, f64)>);

impl Track {
    pub fn new(v: f64) -> Self {
        Track(vec![(0.0, v)])
    }
    pub fn last(&self) -> f64 {
        self.0.last().unwrap().1
    }
    /// Hold the current value until `t`.
    pub fn hold(&mut self, t: f64) {
        let (lt, lv) = *self.0.last().unwrap();
        if t > lt {
            self.0.push((t, lv));
        }
    }
    /// From `t`, pass through (fraction of `dur`, value) points, linearly.
    pub fn keys(&mut self, t: f64, dur: f64, points: &[(f64, f64)]) {
        self.hold(t);
        for &(f, v) in points {
            self.0.push((t + f * dur, v));
        }
    }
    /// Change to `v` at `t`, instantly.
    pub fn set(&mut self, t: f64, v: f64) {
        self.keys(t, 0.0, &[(0.0, v)]);
    }
    /// From `t`, spring from `from` to `to` over `dur`.
    pub fn spring(&mut self, t: f64, dur: f64, from: f64, to: f64) {
        let n = (SPRING.len() - 1) as f64;
        let points: Vec<(f64, f64)> =
            SPRING.iter().enumerate().map(|(k, s)| (k as f64 / n, from + (to - from) * s)).collect();
        self.keys(t, dur, &points);
    }
    /// From `t`, ease in and out to `to` over `dur`.
    pub fn ease(&mut self, t: f64, dur: f64, to: f64) {
        let from = self.last();
        let smooth = |f: f64| f * f * (3.0 - 2.0 * f);
        let points: Vec<(f64, f64)> =
            (0..=8).map(|k| k as f64 / 8.0).map(|f| (f, from + (to - from) * smooth(f))).collect();
        self.keys(t, dur, &points);
    }
    pub fn frames(&self, f: impl Fn(f64) -> String) -> Vec<(f64, String)> {
        self.0.iter().map(|&(t, v)| (t, f(v))).collect()
    }
    pub fn changes(&self) -> bool {
        self.0.iter().any(|&(_, v)| v != self.0[0].1)
    }
    pub fn svg(&self, attr: &str, total: f64) -> String {
        if self.changes() {
            smil(attr, &self.frames(|v| format!("{v:.2}")), total)
        } else {
            String::new()
        }
    }
}

/// A colour as keyframes, held between changes and blended across `to`.
pub struct Colour(pub Vec<(f64, String)>);

impl Colour {
    pub fn new(c: &str) -> Self {
        Colour(vec![(0.0, c.to_string())])
    }
    /// Hold until `t`, then blend to `c` by `t + dur` (0: a cut).
    pub fn to(&mut self, t: f64, dur: f64, c: &str) {
        let last = self.0.last().unwrap().1.clone();
        if last != c {
            self.0.push((t, last));
            self.0.push((t + dur, c.to_string()));
        }
    }
}

/// A square of side `size` scaled by `scale` about (cx, cy): its x, y, width
/// and height animations (nothing if the scale never changes).
pub fn scaled(cx: f64, cy: f64, size: f64, scale: &Track, total: f64) -> String {
    if !scale.changes() {
        return String::new();
    }
    let at = |off: f64, k: f64| format!("{:.1}", off - size * k / 2.0);
    let len = |k: f64| format!("{:.1}", size * k);
    [
        smil("x", &scale.frames(|k| at(cx, k)), total),
        smil("y", &scale.frames(|k| at(cy, k)), total),
        smil("width", &scale.frames(len), total),
        smil("height", &scale.frames(len), total),
    ]
    .concat()
}
