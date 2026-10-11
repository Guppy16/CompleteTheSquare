//! An animated SVG of a game, for the README, with the page's motion: before
//! each move the engine's top candidates grow in as see-through squares, the
//! chosen one shrinks into the piece as it lands with a spring, the others
//! fade, captured pieces swell and fade rippling out from the move, an eval
//! bar (green's share) springs to the engine's score, and a win pulses its
//! corners, traces a frame round the square and dims the rest. Loops. GitHub
//! plays SVG (SMIL) animations inline in Markdown.
//!
//! The timings are the page's (style.css, script.js) times its --motion, and
//! the spring is the same sampled curve. SMIL has no linear() easing, so the
//! curve is written out as keyframes, and sizes are animated as x/y/width/
//! height rather than transforms, which every SVG renderer plays.
//!
//!     cargo run --release --example animate_game -- "1. A1 E1 2. A2 D1 ..." > docs/img/game.svg

use complete_the_square_ai::game::{play_move, tables, State};
use complete_the_square_ai::search::{analyse_position, TranspositionTable};

/// The page's --motion: every duration and delay below is the page's times this.
const MOTION: f64 = 0.75;
/// The page's --spring (stiffness 170, damping 16), sampled.
const SPRING: [f64; 21] = [
    0.0, 0.103, 0.326, 0.572, 0.784, 0.938, 1.033, 1.077, 1.087, 1.076, 1.056, 1.034, 1.017, 1.004, 0.996,
    0.992, 0.993, 0.995, 0.997, 0.999, 1.0,
];

const PLY: f64 = 1.3; // seconds per move
const DECIDE: f64 = 0.6 * MOTION; // candidates show this long before the piece lands (CANDIDATES_MS)
const GROW: f64 = 0.24 * MOTION; // a candidate growing in
const GROW_STEP: f64 = 0.055 * MOTION; // between candidates, best first
const DROP: f64 = 0.5 * MOTION; // a piece landing
const CHOSEN: f64 = 0.4 * MOTION; // the chosen candidate shrinking into the piece
const FADE: f64 = 0.3 * MOTION; // a captured piece or dismissed candidate fading
const RIPPLE_START: f64 = 0.06 * MOTION; // the first capture after the move
const RIPPLE_STEP: f64 = 0.07 * MOTION; // each step further from the move
const BAR: f64 = 0.6 * MOTION; // the eval bar's spring
const PULSE: f64 = 0.4 * MOTION; // a winning corner's pulse
const PULSE_STEP: f64 = 0.09 * MOTION; // between corners, clockwise from the top left
const TRACE_DELAY: f64 = 0.36 * MOTION; // the frame starts after the corners
const TRACE: f64 = 0.6 * MOTION; // the frame drawing itself
const HOLD: f64 = 3.0; // seconds on the final position
const RESET: f64 = 0.6; // fading back to the empty board

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

/// One animated value as keyframes: held flat between changes.
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
    /// From `t`, jump to `from` and spring to `to` over `dur`.
    fn spring(&mut self, t: f64, dur: f64, from: f64, to: f64) {
        let n = SPRING.len() - 1;
        let points: Vec<(f64, f64)> =
            SPRING.iter().enumerate().map(|(k, s)| (k as f64 / n as f64, from + (to - from) * s)).collect();
        self.keys(t, dur, &points);
    }
    /// From `t`, ease out to `to` over `dur`.
    fn ease_out(&mut self, t: f64, dur: f64, to: f64) {
        let from = self.last();
        let points: Vec<(f64, f64)> =
            (0..=6).map(|k| k as f64 / 6.0).map(|f| (f, from + (to - from) * (1.0 - (1.0 - f).powi(2)))).collect();
        self.keys(t, dur, &points);
    }
    fn frames(&self, f: impl Fn(f64) -> String) -> Vec<(f64, String)> {
        self.0.iter().map(|&(t, v)| (t, f(v))).collect()
    }
    fn changes(&self) -> bool {
        self.0.iter().any(|&(_, v)| v != self.0[0].1)
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

/// A square of `size` (scaled by `scale`) centred on (cx, cy): x, y, width and
/// height animations, or nothing if the scale never changes.
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
    let tt = TranspositionTable::new();

    // Play the game, recording each position, the engine's top candidates and
    // their scores before each move, its score after, and the win.
    let mut states = vec![State::new()];
    let mut candidates: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut green_eval = vec![0.0f64];
    let mut history = vec![State::new().key()];
    let mut win_mask = 0u32;
    for &m in &moves {
        let s = *states.last().unwrap();
        let (cands, _) = analyse_position(&s, 8, 600_000, &history, &tt);
        candidates.push(cands.iter().take(3).map(|c| (c.bit.trailing_zeros() as usize, c.score)).collect());
        let (next, won) = play_move(t, 1 << m, &s);
        history.push(next.key());
        states.push(next);
        if let Some(mask) = won {
            win_mask = mask;
            green_eval.push(if next.current == 0 { 1.0 } else { -1.0 });
            break;
        }
        let (after, _) = analyse_position(&next, 8, 600_000, &history, &tt);
        let score = after.first().map_or(0.0, |c| c.score);
        green_eval.push(if next.current == 0 { score } else { -score });
    }
    let n = states.len() - 1;
    let land = |i: usize| i as f64 * PLY + DECIDE; // when move i's piece lands
    let win_at = land(n - 1);
    let reset = land(n - 1) + HOLD;
    let total = reset + RESET + 0.2;

    // The squares: colour, scale and (for the win) opacity.
    let mut colour: Vec<Colour> = (0..25).map(|_| Colour::new(EMPTY)).collect();
    let mut scale: Vec<Track> = (0..25).map(|_| Track::new(1.0)).collect();
    let mut opacity: Vec<Track> = (0..25).map(|_| Track::new(1.0)).collect();
    for i in 0..n {
        let (before, after, m) = (states[i], states[i + 1], moves[i]);
        let t0 = land(i);
        for c in 0..25 {
            let bit = 1u32 << c;
            if before.occupied() & bit == 0 && after.occupied() & bit != 0 {
                // lands with a spring
                colour[c].to(t0, 0.0, if after.boards[0] & bit != 0 { GREEN } else { RED });
                scale[c].spring(t0, DROP, 0.5, 1.0);
            } else if before.occupied() & bit != 0 && after.occupied() & bit == 0 {
                // captured: swells in its colour, then fades, rippling from the move
                let tc = t0 + RIPPLE_START + RIPPLE_STEP * (distance(m, c) as f64 - 1.0);
                colour[c].to(tc + 0.3 * FADE, 0.7 * FADE, EMPTY);
                scale[c].keys(tc, FADE, &[(0.3, 1.12), (1.0, 1.0)]);
            }
        }
    }

    // The win: corners pulse clockwise from the top left (the corner just
    // played lands instead), the rest of the board dims.
    let corners: Vec<usize> = (0..25).filter(|c| win_mask & (1 << c) != 0).collect();
    if let (Some(&top_left), Some(&bottom_right)) = (corners.first(), corners.last()) {
        let (top, right) = (top_left / 5, bottom_right % 5);
        for &c in &corners {
            if c == moves[n - 1] {
                continue;
            }
            let step = match (c / 5 == top, c % 5 == right) {
                (true, false) => 0.0,
                (true, true) => 1.0,
                (false, true) => 2.0,
                (false, false) => 3.0,
            };
            scale[c].keys(win_at + PULSE_STEP * step, PULSE, &[(0.4, 1.18), (1.0, 1.0)]);
        }
        for c in (0..25).filter(|c| win_mask & (1 << c) == 0) {
            opacity[c].keys(win_at + TRACE_DELAY, 0.4 * MOTION, &[(1.0, 0.4)]);
            opacity[c].keys(reset, RESET, &[(1.0, 1.0)]);
        }
    }
    for c in 0..25 {
        colour[c].to(reset, RESET, EMPTY);
    }

    let board = 5.0 * CELL;
    let width = PAD + board + 70.0;
    let height = PAD + board + 56.0;
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
    let centre = |c: usize| (PAD + (c % 5) as f64 * CELL + CELL / 2.0, PAD + (c / 5) as f64 * CELL + CELL / 2.0);
    for c in 0..25 {
        let (cx, cy) = centre(c);
        let dim = if opacity[c].changes() { smil("opacity", &opacity[c].frames(|v| format!("{v:.2}")), total) } else { String::new() };
        out.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{SQUARE:.0}\" height=\"{SQUARE:.0}\" fill=\"{EMPTY}\">{}{}{dim}</rect>\n",
            cx - SQUARE / 2.0,
            cy - SQUARE / 2.0,
            smil("fill", &colour[c].0, total),
            scaled(cx, cy, SQUARE, &scale[c], total),
        ));
    }

    // The candidates: see-through squares in the mover's colour, larger and
    // stronger the closer each is to the best (as on the page). The move
    // played is shown as the best.
    for (i, cands) in candidates.iter().enumerate() {
        let t0 = land(i) - DECIDE;
        let best = cands.first().map_or(0.0, |&(_, s)| s);
        let mut shown = vec![(moves[i], best)];
        shown.extend(cands.iter().copied().filter(|&(c, _)| c != moves[i]));
        let fill = if i % 2 == 0 { GREEN } else { RED };
        for (rank, &(c, score)) in shown.iter().enumerate() {
            let gap = (best - score).abs();
            let size = 0.6 * SQUARE * (1.0 - gap * 4.0).max(0.45);
            let peak = 0.6 * (1.0 - gap * 3.0).max(0.3);
            let start = t0 + GROW_STEP * rank as f64;
            let (mut k, mut op) = (Track::new(0.2), Track::new(0.0));
            k.ease_out(start, GROW, 1.0);
            op.ease_out(start, GROW, peak);
            if rank == 0 {
                k.ease_out(land(i), CHOSEN, 0.5);
                op.ease_out(land(i), CHOSEN, 0.0);
            } else {
                k.keys(land(i), FADE, &[(0.3, 1.12), (1.0, 0.8)]);
                op.keys(land(i), FADE, &[(0.3, peak), (1.0, 0.0)]);
            }
            let (cx, cy) = centre(c);
            out.push_str(&format!(
                "<rect x=\"{cx:.0}\" y=\"{cy:.0}\" width=\"0\" height=\"0\" fill=\"{fill}\" opacity=\"0\">{}{}</rect>\n",
                scaled(cx, cy, size, &k, total),
                smil("opacity", &op.frames(|v| format!("{v:.3}")), total)
            ));
        }
    }

    // The last move: a soft white inner outline, as on the page.
    for (i, &m) in moves.iter().take(n).enumerate() {
        let mut op = Track::new(0.0);
        op.keys(land(i), 0.0, &[(0.0, 1.0)]);
        if i + 1 < n {
            op.keys(land(i + 1), 0.0, &[(0.0, 0.0)]);
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
            smil("opacity", &op.frames(|v| format!("{v:.2}")), total)
        ));
    }

    // The win's frame: drawn round the square's corners, just outside them.
    if let (Some(&first), Some(&last)) = (corners.first(), corners.last()) {
        let (x0, y0) = (centre(first).0 - SQUARE / 2.0 - 5.0, centre(first).1 - SQUARE / 2.0 - 5.0);
        let (x1, y1) = (centre(last).0 + SQUARE / 2.0 + 5.0, centre(last).1 + SQUARE / 2.0 + 5.0);
        let start = win_at + TRACE_DELAY;
        let mut dash = Track::new(100.0);
        dash.ease_out(start, TRACE, 0.0);
        let mut op = Track::new(0.0);
        op.keys(start, 0.0, &[(0.0, 1.0)]);
        op.keys(reset, RESET, &[(1.0, 0.0)]);
        out.push_str(&format!(
            "<rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"{ACCENT}\" stroke-width=\"3\" stroke-linecap=\"square\" pathLength=\"100\" stroke-dasharray=\"100\" stroke-dashoffset=\"100\" opacity=\"0\">{}{}</rect>\n",
            x1 - x0,
            y1 - y0,
            smil("stroke-dashoffset", &dash.frames(|v| format!("{v:.1}")), total),
            smil("opacity", &op.frames(|v| format!("{v:.2}")), total)
        ));
    }

    // Eval bar to the right: green's share from the bottom, springing to each
    // new score. Square-cornered, as on the page.
    let bx = PAD + board + 22.0;
    let (by, bh) = (PAD + 2.0, board - 4.0);
    out.push_str(&format!("<rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"14\" height=\"{bh:.0}\" fill=\"{RED}\"/>\n"));
    let mut share = Track::new(bh / 2.0);
    for (i, e) in green_eval.iter().enumerate().skip(1) {
        let to = (0.5 + 0.5 * e.clamp(-1.0, 1.0)) * bh;
        share.spring(land(i - 1) + 0.1, BAR, share.last(), to);
    }
    share.ease_out(reset, RESET, bh / 2.0);
    let clamped = |v: f64| v.clamp(0.0, bh);
    out.push_str(&format!(
        "<rect x=\"{bx:.0}\" y=\"{:.1}\" width=\"14\" height=\"{:.1}\" fill=\"{GREEN}\">{}{}</rect>\n",
        by + bh / 2.0,
        bh / 2.0,
        smil("height", &share.frames(|v| format!("{:.1}", clamped(v))), total),
        smil("y", &share.frames(|v| format!("{:.1}", by + bh - clamped(v))), total)
    ));

    // Caption: the move just played, one text per ply.
    let cap_y = PAD + board + 30.0;
    for i in 0..n {
        let ply = i + 1;
        let num = if i % 2 == 0 { format!("{}.", i / 2 + 1) } else { format!("{}...", i / 2 + 1) };
        let side = if i % 2 == 0 { "green" } else { "red" };
        let captured = states[i].occupied() & !states[ply].occupied() & !(1u32 << moves[i]);
        let lost: Vec<String> = (0..25).filter(|c| captured & (1 << c) != 0).map(name).collect();
        let mut caption = format!("{num} {side} {}", name(moves[i]));
        if !lost.is_empty() {
            caption.push_str(&format!(" captures {}", lost.join(", ")));
        }
        if ply == n && win_mask != 0 {
            caption = format!("{num} {side} {} completes the square", name(moves[i]));
        }
        let mut op = Track::new(0.0);
        op.keys(land(i), 0.05, &[(1.0, 1.0)]);
        let off = if ply == n { reset } else { land(i + 1) };
        op.keys(off, 0.05, &[(1.0, 0.0)]);
        out.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{cap_y:.0}\" fill=\"#e6e6e6\" font-size=\"14\" text-anchor=\"middle\" opacity=\"0\">{caption}{}</text>\n",
            PAD + board / 2.0,
            smil("opacity", &op.frames(|v| format!("{v:.2}")), total)
        ));
    }
    out.push_str("</svg>\n");
    print!("{out}");
}
