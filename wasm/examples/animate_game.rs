//! An animated SVG of a short game, for the README, with the page's motion: a
//! cursor plays each move, and for the first few captures it slows down to
//! show the hover preview (it hovers a neighbouring square, then moves on to
//! the one it plays with the frame springing across, and lingers on the
//! capture hints) before it clicks; the piece lands with a spring and captured
//! pieces swell and fade. A win pulses its corners, traces a frame round the square and dims
//! the rest. Above the board, the rules appear one at a time as the game reaches them:
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

#[path = "common/anim.rs"]
mod anim;
use anim::*;

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
const TEXT_WIDTH: f64 = 380.0; // the picture's width: the longest rule fits
const TEXT_HEIGHT: f64 = 52.0; // the rule and the move, above the board
const PHRASE_FADE: f64 = 0.3; // one rule fading out as the next fades in

const RULES: [&str; 3] =
    ["Place a piece on an empty square", "Capture and remove sandwiched pieces", "Complete the corners of a square to win"];

/// Moves a king would need between two squares: the steps along a capture ray.
fn distance(a: usize, b: usize) -> usize {
    (a / 5).abs_diff(b / 5).max((a % 5).abs_diff(b % 5))
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
    // The text goes above the board, which is centred below it: wide enough
    // for the longest rule.
    let width = TEXT_WIDTH;
    let (dx, dy) = ((width - (PAD + board + PAD)) / 2.0, TEXT_HEIGHT);
    let height = dy + PAD + board + 40.0; // room for the cursor resting below the board
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" font-family=\"system-ui, sans-serif\">\n<rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"#121212\"/>\n<g transform=\"translate({dx:.0} {dy:.0})\">\n"
    );
    let mut text = String::new();
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
        text.push_str(&format!(
            "<text x=\"{:.0}\" y=\"28\" fill=\"#e6e6e6\" font-size=\"15\" font-weight=\"600\" text-anchor=\"middle\" opacity=\"{}\">{rule}{}</text>\n",
            width / 2.0,
            if k == 0 { 1 } else { 0 },
            op.svg("opacity", total)
        ));
    }

    // Under the rule: the move just played.
    let cap_y = 50.0;
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
        text.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{cap_y:.0}\" fill=\"#8a8a8a\" font-size=\"13\" text-anchor=\"middle\" opacity=\"0\">{caption}{}</text>\n",
            width / 2.0,
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
    out.push_str("</g>\n");
    out.push_str(&text);
    out.push_str("</svg>\n");
    print!("{out}");
}
