//! An animated SVG of a short game, for the README, with the page's motion: a
//! cursor plays each move, and for the first few captures it slows down to
//! show the hover preview (it hovers a neighbouring square, then moves on to
//! the one it plays with the frame springing across, and lingers on the
//! capture hints) before it clicks; the piece lands with a spring and captured
//! pieces swell and fade. A win pulses its corners, traces a frame round the square and dims
//! the rest. Under the board, the rules appear one at a time as the game reaches them:
//! placing, from the start; capturing, from the first capture; winning, from the move
//! that makes the last threat. Loops. GitHub plays SVG (SMIL) animations inline in Markdown.
//!
//! Timings are the page's (style.css) times its --motion, and the spring is
//! the same sampled curve. SMIL has no linear() easing, so the curve is written
//! out as keyframes, and sizes are animated as x/y/width/height rather than
//! transforms.
//!
//!     cargo run --release --example animate_game -- "1. A1 B2 2. C3 E5 3. D4 B2 4. A4 C3 5. D4 C3 6. D4 C5 7. D1" > docs/img/how-to-play.svg

use complete_the_square_ai::game::{play_move, tables, State};

/// The page's --motion: every page duration below is scaled by it.
const MOTION: f64 = 0.75;
/// The page's --spring (stiffness 170, damping 16), sampled.
const SPRING: [f64; 21] = [
    0.0, 0.103, 0.326, 0.572, 0.784, 0.938, 1.033, 1.077, 1.087, 1.076, 1.056, 1.034, 1.017, 1.004, 0.996,
    0.992, 0.993, 0.995, 0.997, 0.999, 1.0,
];

// The page's timings.
const DROP: f64 = 0.5 * MOTION; // a piece landing, a ghost springing in
const SETTLE: f64 = 0.45 * MOTION; // the hover frame and capture hints springing
const FADE: f64 = 0.3 * MOTION; // a captured piece fading
const RIPPLE_START: f64 = 0.06 * MOTION; // the first capture after the move
const RIPPLE_STEP: f64 = 0.07 * MOTION; // each step further from the move
const FRAME_FADE: f64 = 0.15 * MOTION; // the hover frame appearing or hiding
const PULSE: f64 = 0.4 * MOTION; // a winning corner's pulse
const PULSE_STEP: f64 = 0.09 * MOTION; // between corners, clockwise from the top left
const TRACE_DELAY: f64 = 0.36 * MOTION; // the win's frame starts after the corners
const TRACE: f64 = 0.6 * MOTION; // the win's frame drawing itself

// The cursor's pace (not on the page: this is the player).
const SLOW_CAPTURES: usize = 3; // the first this many captures are played slowly
const START: f64 = 1.0; // before the first move, long enough to read the first rule
const TRAVEL: f64 = 0.3; // gliding to the first square it hovers
const QUICK_AIM: f64 = 0.15; // a quick move: hovering its square before the click
const LOOK: f64 = 0.45; // a slow move: hovering a neighbouring square
const SLIDE: f64 = 0.22; // moving on to the square it plays
const AIM: f64 = 0.9; // lingering on the capture hints before the click
const AFTER: f64 = 0.3; // after the click, before the next move
const HOLD: f64 = 2.5; // on the final position
const RESET: f64 = 0.6; // fading back to the empty board
const PHRASE_FADE: f64 = 0.3; // one rule fading out as the next fades in

const RULES: [&str; 3] =
    ["Place a piece on an empty square", "Capture and remove sandwiched pieces", "Complete the corners of a square to win"];

const CELL: f64 = 44.0;
const PAD: f64 = 28.0;
const SQUARE: f64 = CELL - 4.0; // a square's drawn size, inside the grid gap

const EMPTY: &str = "#2a2a2a";
const GREEN: &str = "#4CAF50";
const RED: &str = "#F44336";
const ACCENT: &str = "#ff9900";

fn name(i: usize) -> String {
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}

/// Moves a king would need between two squares: the steps along a capture ray.
fn distance(a: usize, b: usize) -> usize {
    (a / 5).abs_diff(b / 5).max((a % 5).abs_diff(b % 5))
}

fn centre(c: usize) -> (f64, f64) {
    (PAD + (c % 5) as f64 * CELL + CELL / 2.0, PAD + (c / 5) as f64 * CELL + CELL / 2.0)
}

/// SMIL keyTimes/values for one attribute, from (seconds, value) keyframes in
/// time order. Equal times are nudged apart (an instant change).
fn smil(attr: &str, frames: &[(f64, String)], total: f64) -> String {
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
struct Track(Vec<(f64, f64)>);

impl Track {
    fn new(v: f64) -> Self {
        Track(vec![(0.0, v)])
    }
    fn last(&self) -> f64 {
        self.0.last().unwrap().1
    }
    /// Hold the current value until `t`.
    fn hold(&mut self, t: f64) {
        let (lt, lv) = *self.0.last().unwrap();
        if t > lt {
            self.0.push((t, lv));
        }
    }
    /// From `t`, pass through (fraction of `dur`, value) points, linearly.
    fn keys(&mut self, t: f64, dur: f64, points: &[(f64, f64)]) {
        self.hold(t);
        for &(f, v) in points {
            self.0.push((t + f * dur, v));
        }
    }
    /// Change to `v` at `t`, instantly.
    fn set(&mut self, t: f64, v: f64) {
        self.keys(t, 0.0, &[(0.0, v)]);
    }
    /// From `t`, spring from `from` to `to` over `dur`.
    fn spring(&mut self, t: f64, dur: f64, from: f64, to: f64) {
        let n = (SPRING.len() - 1) as f64;
        let points: Vec<(f64, f64)> =
            SPRING.iter().enumerate().map(|(k, s)| (k as f64 / n, from + (to - from) * s)).collect();
        self.keys(t, dur, &points);
    }
    /// From `t`, ease in and out to `to` over `dur`.
    fn ease(&mut self, t: f64, dur: f64, to: f64) {
        let from = self.last();
        let smooth = |f: f64| f * f * (3.0 - 2.0 * f);
        let points: Vec<(f64, f64)> =
            (0..=8).map(|k| k as f64 / 8.0).map(|f| (f, from + (to - from) * smooth(f))).collect();
        self.keys(t, dur, &points);
    }
    fn frames(&self, f: impl Fn(f64) -> String) -> Vec<(f64, String)> {
        self.0.iter().map(|&(t, v)| (t, f(v))).collect()
    }
    fn changes(&self) -> bool {
        self.0.iter().any(|&(_, v)| v != self.0[0].1)
    }
    fn svg(&self, attr: &str, total: f64) -> String {
        if self.changes() {
            smil(attr, &self.frames(|v| format!("{v:.2}")), total)
        } else {
            String::new()
        }
    }
}

/// A colour as keyframes, held between changes and blended across `to`.
struct Colour(Vec<(f64, String)>);

impl Colour {
    fn new(c: &str) -> Self {
        Colour(vec![(0.0, c.to_string())])
    }
    /// Hold until `t`, then blend to `c` by `t + dur` (0: a cut).
    fn to(&mut self, t: f64, dur: f64, c: &str) {
        let last = self.0.last().unwrap().1.clone();
        if last != c {
            self.0.push((t, last));
            self.0.push((t + dur, c.to_string()));
        }
    }
}

/// A square of side `size` scaled by `scale` about (cx, cy): its x, y, width
/// and height animations (nothing if the scale never changes).
fn scaled(cx: f64, cy: f64, size: f64, scale: &Track, total: f64) -> String {
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

/// An empty square next to `target`, the one nearest `from`: where the
/// cursor hovers before moving on to the square it plays.
fn neighbour(state: &State, target: usize, from: (f64, f64)) -> usize {
    let (tr, tc) = ((target / 5) as i32, (target % 5) as i32);
    let mut best = target;
    let mut best_d = f64::INFINITY;
    for (dr, dc) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
        let (r, c) = (tr + dr, tc + dc);
        if !(0..5).contains(&r) || !(0..5).contains(&c) {
            continue;
        }
        let sq = (r * 5 + c) as usize;
        if state.occupied() & (1 << sq) != 0 {
            continue;
        }
        let (x, y) = centre(sq);
        let d = (x - from.0).hypot(y - from.1);
        if d < best_d {
            best = sq;
            best_d = d;
        }
    }
    best
}

fn main() {
    let moves_arg = std::env::args().nth(1).expect("pass a move list");
    let moves: Vec<usize> = moves_arg
        .split_whitespace()
        .filter(|w| !w.ends_with('.'))
        .map(|w| {
            let b = w.as_bytes();
            (b[1] - b'1') as usize * 5 + (b[0].to_ascii_uppercase() - b'A') as usize
        })
        .collect();
    let t = tables();

    let mut states = vec![State::new()];
    let mut win_mask = 0u32;
    for &m in &moves {
        let (next, won) = play_move(t, 1 << m, states.last().unwrap());
        states.push(next);
        if let Some(mask) = won {
            win_mask = mask;
            break;
        }
    }
    let n = states.len() - 1;
    // What a move at `sq` would capture from `state`.
    let captures = |state: &State, sq: usize| {
        let theirs = state.boards[1 - state.current];
        theirs & !play_move(t, 1 << sq, state).0.boards[1 - state.current]
    };

    let mut colour: Vec<Colour> = (0..25).map(|_| Colour::new(EMPTY)).collect();
    let mut scale: Vec<Track> = (0..25).map(|_| Track::new(1.0)).collect();
    let mut dim: Vec<Track> = (0..25).map(|_| Track::new(1.0)).collect();
    let rest = (PAD + 5.0 * CELL - 8.0, PAD + 5.0 * CELL + 14.0); // the cursor between loops
    let (mut cursor_x, mut cursor_y) = (Track::new(rest.0), Track::new(rest.1));
    let (mut frame_x, mut frame_y, mut frame_op) = (Track::new(0.0), Track::new(0.0), Track::new(0.0));
    let mut ghosts = Vec::new(); // (square, colour, appear, leave)
    let mut hints = Vec::new(); // (square, appear, leave)
    let mut clicks = Vec::new();
    let mut starts = Vec::new(); // when the cursor sets off for each move
    let mut now = START;
    let mut slow_left = SLOW_CAPTURES;

    for i in 0..n {
        let (before, after, m) = (states[i], states[i + 1], moves[i]);
        starts.push(now);
        let mover = if before.current == 0 { GREEN } else { RED };
        let slow = slow_left > 0 && captures(&before, m) != 0;
        let first = if slow { neighbour(&before, m, (cursor_x.last(), cursor_y.last())) } else { m };
        if slow {
            slow_left -= 1;
        }
        let hovers = if first == m { vec![(m, TRAVEL)] } else { vec![(first, TRAVEL), (m, SLIDE)] };
        for (k, &(sq, travel)) in hovers.iter().enumerate() {
            let (x, y) = centre(sq);
            cursor_x.ease(now, travel, x + 4.0);
            cursor_y.ease(now, travel, y + 4.0);
            let arrive = now + travel;
            // The frame fades in where it is if it was hidden, else springs across.
            let (fx, fy) = (x - SQUARE / 2.0, y - SQUARE / 2.0);
            if frame_op.last() == 0.0 {
                frame_x.set(arrive, fx);
                frame_y.set(arrive, fy);
                frame_op.keys(arrive, FRAME_FADE, &[(1.0, 0.7)]);
            } else {
                frame_x.spring(arrive, SETTLE, frame_x.last(), fx);
                frame_y.spring(arrive, SETTLE, frame_y.last(), fy);
            }
            let leave = arrive + if k + 1 < hovers.len() { LOOK } else if slow { AIM } else { QUICK_AIM };
            ghosts.push((sq, mover, arrive, leave));
            let hinted = captures(&before, sq);
            for c in (0..25).filter(|c| hinted & (1 << c) != 0) {
                hints.push((c, arrive, leave));
                let from = scale[c].last();
                scale[c].spring(arrive, SETTLE, from, 0.8);
                if sq != m {
                    scale[c].spring(leave, SETTLE, 0.8, 1.0);
                }
            }
            now = leave;
        }
        // The click: the piece lands, captures ripple out, the frame hides.
        let click = now;
        clicks.push(click);
        frame_op.keys(click, FRAME_FADE, &[(1.0, 0.0)]);
        for c in 0..25 {
            let bit = 1u32 << c;
            if before.occupied() & bit == 0 && after.occupied() & bit != 0 {
                colour[c].to(click, 0.0, mover);
                scale[c].spring(click, DROP, 0.5, 1.0);
            } else if before.occupied() & bit != 0 && after.occupied() & bit == 0 {
                let tc = click + RIPPLE_START + RIPPLE_STEP * (distance(m, c) as f64 - 1.0);
                let from = scale[c].last();
                colour[c].to(tc + 0.3 * FADE, 0.7 * FADE, EMPTY);
                scale[c].keys(tc, FADE, &[(0.0, from), (0.3, 1.12), (1.0, 1.0)]);
            }
        }
        now = click + AFTER;
    }

    // The win: corners pulse clockwise from the top left (the corner just
    // played lands instead), the rest of the board dims, a frame is traced.
    let win_at = *clicks.last().unwrap();
    let reset = win_at + HOLD;
    let corners: Vec<usize> = (0..25).filter(|c| win_mask & (1 << c) != 0).collect();
    if let (Some(&top_left), Some(&bottom_right)) = (corners.first(), corners.last()) {
        let (top, right) = (top_left / 5, bottom_right % 5);
        for &c in corners.iter().filter(|&&c| c != moves[n - 1]) {
            let step = match (c / 5 == top, c % 5 == right) {
                (true, false) => 0.0,
                (true, true) => 1.0,
                (false, true) => 2.0,
                (false, false) => 3.0,
            };
            scale[c].keys(win_at + PULSE_STEP * step, PULSE, &[(0.4, 1.18), (1.0, 1.0)]);
        }
        for c in (0..25).filter(|c| win_mask & (1 << c) == 0) {
            dim[c].keys(win_at + TRACE_DELAY, 0.4 * MOTION, &[(1.0, 0.4)]);
            dim[c].keys(reset, RESET, &[(1.0, 1.0)]);
        }
    }
    for c in colour.iter_mut() {
        c.to(reset, RESET, EMPTY);
    }
    cursor_x.ease(reset, RESET, rest.0);
    cursor_y.ease(reset, RESET, rest.1);
    let total = reset + RESET + PHRASE_FADE;

    let board = 5.0 * CELL;
    let width = PAD + board + PAD;
    let height = PAD + board + 66.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" font-family=\"system-ui, sans-serif\">\n<rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"#121212\"/>\n"
    );
    for c in 0..5 {
        out.push_str(&format!(
            "<text x=\"{:.0}\" y=\"18\" fill=\"#8a8a8a\" font-size=\"13\" text-anchor=\"middle\">{}</text>\n",
            PAD + c as f64 * CELL + CELL / 2.0,
            (b'A' + c as u8) as char
        ));
        out.push_str(&format!(
            "<text x=\"12\" y=\"{:.0}\" fill=\"#8a8a8a\" font-size=\"13\" text-anchor=\"middle\">{}</text>\n",
            PAD + c as f64 * CELL + CELL / 2.0 + 5.0,
            c + 1
        ));
    }
    for c in 0..25 {
        let (cx, cy) = centre(c);
        out.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{SQUARE:.0}\" height=\"{SQUARE:.0}\" fill=\"{EMPTY}\">{}{}{}</rect>\n",
            cx - SQUARE / 2.0,
            cy - SQUARE / 2.0,
            smil("fill", &colour[c].0, total),
            scaled(cx, cy, SQUARE, &scale[c], total),
            dim[c].svg("opacity", total),
        ));
    }

    // Ghosts: a faint piece springing into each hovered square.
    for &(sq, fill, appear, leave) in &ghosts {
        let (cx, cy) = centre(sq);
        let mut k = Track::new(0.5);
        k.spring(appear, DROP, 0.5, 1.0);
        let mut op = Track::new(0.0);
        op.set(appear, 0.3);
        op.set(leave, 0.0);
        out.push_str(&format!(
            "<rect x=\"{cx:.0}\" y=\"{cy:.0}\" width=\"0\" height=\"0\" fill=\"{fill}\" opacity=\"0\">{}{}</rect>\n",
            scaled(cx, cy, SQUARE, &k, total),
            op.svg("opacity", total)
        ));
    }
    // Capture hints: a dashed outline round each piece the hovered move would take.
    for &(c, appear, leave) in &hints {
        let (cx, cy) = centre(c);
        let side = SQUARE * 0.8 + 6.0;
        let mut op = Track::new(0.0);
        op.set(appear, 1.0);
        op.set(leave, 0.0);
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{side:.1}\" height=\"{side:.1}\" fill=\"none\" stroke=\"{ACCENT}\" stroke-width=\"2\" stroke-dasharray=\"5 3\" opacity=\"0\">{}</rect>\n",
            cx - side / 2.0,
            cy - side / 2.0,
            op.svg("opacity", total)
        ));
    }
    // The last move: a soft white inner outline, as on the page.
    for (i, &m) in moves.iter().take(n).enumerate() {
        let mut op = Track::new(0.0);
        op.set(clicks[i], 1.0);
        if i + 1 < n {
            op.set(clicks[i + 1], 0.0);
        } else {
            op.keys(reset, RESET, &[(1.0, 0.0)]);
        }
        let (cx, cy) = centre(m);
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"#ffffff\" stroke-opacity=\"0.45\" stroke-width=\"3\" opacity=\"0\">{}</rect>\n",
            cx - SQUARE / 2.0 + 1.5,
            cy - SQUARE / 2.0 + 1.5,
            SQUARE - 3.0,
            SQUARE - 3.0,
            op.svg("opacity", total)
        ));
    }

    // The hover frame, springing between squares.
    out.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{SQUARE:.0}\" height=\"{SQUARE:.0}\" fill=\"none\" stroke=\"#e6e6e6\" stroke-width=\"2\" opacity=\"0\">{}{}{}</rect>\n",
        frame_x.svg("x", total),
        frame_y.svg("y", total),
        frame_op.svg("opacity", total)
    ));

    // The win's frame: traced round the square's corners, just outside them.
    if let (Some(&first), Some(&last)) = (corners.first(), corners.last()) {
        let (x0, y0) = (centre(first).0 - SQUARE / 2.0 - 5.0, centre(first).1 - SQUARE / 2.0 - 5.0);
        let (x1, y1) = (centre(last).0 + SQUARE / 2.0 + 5.0, centre(last).1 + SQUARE / 2.0 + 5.0);
        let start = win_at + TRACE_DELAY;
        let mut dash = Track::new(100.0);
        dash.ease(start, TRACE, 0.0);
        let mut op = Track::new(0.0);
        op.set(start, 1.0);
        op.keys(reset, RESET, &[(1.0, 0.0)]);
        out.push_str(&format!(
            "<rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"{ACCENT}\" stroke-width=\"3\" stroke-linecap=\"square\" pathLength=\"100\" stroke-dasharray=\"100\" stroke-dashoffset=\"100\" opacity=\"0\">{}{}</rect>\n",
            x1 - x0,
            y1 - y0,
            dash.svg("stroke-dashoffset", total),
            op.svg("opacity", total)
        ));
    }

    // The rules, one at a time: capturing from the cursor setting off for the
    // first capture, winning from the winner's move that last made three of the
    // four corners theirs (the threat that stands until the win).
    let first_capture = (0..n).find(|&i| states[i].occupied() & !states[i + 1].occupied() != 0);
    let winner = states[n].current;
    let threat = (0..n)
        .rev()
        .filter(|&i| states[i].current == winner)
        .find(|&i| (states[i].boards[winner] & win_mask).count_ones() < 3 && i + 1 < n);
    let changes = [first_capture.map(|i| starts[i]), threat.filter(|_| win_mask != 0).map(|i| starts[i])];
    let mut bounds = vec![0.0];
    bounds.extend(changes.iter().flatten());
    bounds.push(reset);
    for (k, rule) in RULES.iter().enumerate().take(bounds.len() - 1) {
        let mut op = Track::new(if k == 0 { 1.0 } else { 0.0 });
        if k > 0 {
            op.keys(bounds[k] + PHRASE_FADE, PHRASE_FADE, &[(1.0, 1.0)]); // after the last one has gone
        }
        op.keys(bounds[k + 1], PHRASE_FADE, &[(1.0, 0.0)]);
        if k == 0 {
            op.keys(reset + RESET, PHRASE_FADE, &[(1.0, 1.0)]); // back for the next loop
        }
        out.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"#e6e6e6\" font-size=\"15\" font-weight=\"600\" text-anchor=\"middle\" opacity=\"{}\">{rule}{}</text>\n",
            PAD + board / 2.0,
            PAD + board + 30.0,
            if k == 0 { 1 } else { 0 },
            op.svg("opacity", total)
        ));
    }

    // Under the rule: the move just played.
    let cap_y = PAD + board + 52.0;
    for i in 0..n {
        let num = if i % 2 == 0 { format!("{}.", i / 2 + 1) } else { format!("{}...", i / 2 + 1) };
        let side = if i % 2 == 0 { "Green" } else { "Red" };
        let captured = states[i].occupied() & !states[i + 1].occupied();
        let lost: Vec<String> = (0..25).filter(|c| captured & (1 << c) != 0).map(name).collect();
        let mut caption = format!("{num} {side} {}", name(moves[i]));
        if !lost.is_empty() {
            caption.push_str(&format!(" captures {}", lost.join(", ")));
        }
        if i + 1 == n && win_mask != 0 {
            caption = format!("{num} {side} {} completes the square", name(moves[i]));
        }
        let mut op = Track::new(0.0);
        op.keys(clicks[i], 0.05, &[(1.0, 1.0)]);
        let off = if i + 1 == n { reset } else { clicks[i + 1] };
        op.keys(off, 0.05, &[(1.0, 0.0)]);
        out.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{cap_y:.0}\" fill=\"#8a8a8a\" font-size=\"13\" text-anchor=\"middle\" opacity=\"0\">{caption}{}</text>\n",
            PAD + board / 2.0,
            op.svg("opacity", total)
        ));
    }

    // The cursor, on top of everything: an arrow whose tip follows the moves.
    // Its x and y tracks are always moved together, so their keyframes pair up.
    let pos: Vec<(f64, String)> =
        cursor_x.0.iter().zip(&cursor_y.0).map(|(&(t, x), &(_, y))| (t, format!("{x:.1} {y:.1}"))).collect();
    let translate = smil("transform", &pos, total)
        .replace("<animate attributeName=\"transform\"", "<animateTransform attributeName=\"transform\" type=\"translate\"");
    out.push_str(&format!(
        "<g transform=\"translate({:.1} {:.1})\"><path d=\"M0 0 L0 17 L4.6 12.6 L7.7 19.4 L10.4 18.2 L7.4 11.6 L13 11.6 Z\" fill=\"#ffffff\" stroke=\"#121212\" stroke-width=\"1.2\" stroke-linejoin=\"round\"/>{translate}</g>\n",
        rest.0, rest.1
    ));
    out.push_str("</svg>\n");
    print!("{out}");
}
