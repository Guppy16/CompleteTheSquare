//! An animated SVG of a game, for the README: before each move the engine's
//! top candidate squares pulse ("thinking"), the piece fades in, captured
//! pieces fade out, an eval bar (green's share) follows the engine's score,
//! and the winning square lights up at the end. Loops. GitHub plays SVG
//! (SMIL) animations inline in Markdown.
//!
//!     cargo run --release --example animate_game -- "1. A1 E1 2. A2 D1 ..." > docs/img/game.svg

use complete_the_square_ai::game::{play_move, tables, State};
use complete_the_square_ai::search::{analyse_position, TranspositionTable};

const PLY: f64 = 1.1; // seconds per move
const HOLD: f64 = 3.0; // seconds on the final position
const CELL: f64 = 44.0;
const PAD: f64 = 28.0;

const EMPTY: &str = "#2a2a2a";
const GREEN: &str = "#4CAF50";
const RED: &str = "#F44336";

fn name(i: usize) -> String {
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}

/// Keyframe list for one animated attribute: (time in seconds, value).
struct Track(Vec<(f64, String)>);

impl Track {
    fn new(v: &str) -> Self {
        Track(vec![(0.0, v.to_string())])
    }
    /// Hold the previous value until `t`, then move to `v` by `t + ease`.
    fn to(&mut self, t: f64, ease: f64, v: &str) {
        let last = self.0.last().unwrap().1.clone();
        if last == v {
            return;
        }
        self.0.push((t, last));
        self.0.push((t + ease, v.to_string()));
    }
    fn svg(&self, attr: &str, total: f64) -> String {
        let mut frames = self.0.clone();
        let end = frames.last().unwrap().1.clone();
        frames.push((total, end));
        let mut times = Vec::new();
        let mut values = Vec::new();
        let mut prev = -1.0;
        for (t, v) in frames {
            let mut k = (t / total).clamp(0.0, 1.0);
            if k <= prev {
                k = prev + 1e-4;
            }
            prev = k;
            times.push(format!("{:.4}", k.min(1.0)));
            values.push(v);
        }
        if let Some(last) = times.last_mut() {
            *last = "1".to_string();
        }
        format!(
            "<animate attributeName=\"{attr}\" dur=\"{total:.2}s\" repeatCount=\"indefinite\" keyTimes=\"{}\" values=\"{}\"/>",
            times.join(";"),
            values.join(";")
        )
    }
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

    // Play the game, recording each position, the engine's view before each
    // move (top candidates and the score), and the win.
    let mut states = vec![State::new()];
    let mut candidates: Vec<Vec<usize>> = Vec::new();
    let mut green_eval = vec![0.0f64];
    let mut history = vec![State::new().key()];
    let mut win_mask = 0u32;
    for &m in &moves {
        let s = *states.last().unwrap();
        let (cands, _) = analyse_position(&s, 8, 600_000, &history, &tt);
        candidates.push(cands.iter().take(3).map(|c| c.bit.trailing_zeros() as usize).collect());
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
    let total = n as f64 * PLY + HOLD + 0.8;

    // Per-cell colour tracks.
    let mut cells: Vec<Track> = (0..25).map(|_| Track::new(EMPTY)).collect();
    for (i, s) in states.iter().enumerate().skip(1) {
        let t0 = (i - 1) as f64 * PLY;
        for c in 0..25 {
            let colour = if s.boards[0] & (1 << c) != 0 {
                GREEN
            } else if s.boards[1] & (1 << c) != 0 {
                RED
            } else {
                EMPTY
            };
            let was_empty_before = states[i - 1].occupied() & (1 << c) == 0;
            // new piece at 0.5s, captures fade out after it lands
            let (start, ease) = if was_empty_before { (t0 + 0.5, 0.18) } else { (t0 + 0.75, 0.3) };
            cells[c].to(start, ease, colour);
        }
    }
    let reset = n as f64 * PLY + HOLD;
    for c in cells.iter_mut() {
        c.to(reset, 0.6, EMPTY);
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
    for (c, track) in cells.iter().enumerate() {
        let (x, y) = (PAD + (c % 5) as f64 * CELL, PAD + (c / 5) as f64 * CELL);
        out.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"3\" fill=\"{EMPTY}\">{}</rect>\n",
            x + 2.0,
            y + 2.0,
            CELL - 4.0,
            CELL - 4.0,
            track.svg("fill", total)
        ));
    }

    // "Thinking": the engine's top candidates pulse before each move.
    for (i, cands) in candidates.iter().enumerate() {
        let t0 = i as f64 * PLY;
        for (rank, &c) in cands.iter().enumerate() {
            let (x, y) = (PAD + (c % 5) as f64 * CELL + CELL / 2.0, PAD + (c / 5) as f64 * CELL + CELL / 2.0);
            let peak = ["0.9", "0.55", "0.35"][rank];
            let mut op = Track::new("0");
            op.to(t0 + 0.02 + rank as f64 * 0.04, 0.18, peak);
            op.to(t0 + 0.42, 0.12, "0");
            out.push_str(&format!(
                "<circle cx=\"{x:.0}\" cy=\"{y:.0}\" r=\"{:.0}\" fill=\"none\" stroke=\"#ff9900\" stroke-width=\"3\" opacity=\"0\">{}</circle>\n",
                CELL / 2.0 - 9.0,
                op.svg("opacity", total)
            ));
        }
    }

    // The last move: a white outline that follows the moves.
    for (i, &m) in moves.iter().take(n).enumerate() {
        let t0 = i as f64 * PLY;
        let mut op = Track::new("0");
        op.to(t0 + 0.5, 0.1, "1");
        op.to(t0 + PLY + 0.5, 0.1, "0");
        let (x, y) = (PAD + (m % 5) as f64 * CELL, PAD + (m / 5) as f64 * CELL);
        out.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"3\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"2.5\" opacity=\"0\">{}</rect>\n",
            x + 2.0,
            y + 2.0,
            CELL - 4.0,
            CELL - 4.0,
            op.svg("opacity", total)
        ));
    }

    // The winning square: its corners glow during the hold.
    for c in (0..25).filter(|c| win_mask & (1 << c) != 0) {
        let mut op = Track::new("0");
        op.to(n as f64 * PLY - 0.1, 0.4, "1");
        op.to(reset, 0.4, "0");
        let (x, y) = (PAD + (c % 5) as f64 * CELL, PAD + (c / 5) as f64 * CELL);
        out.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"3\" fill=\"none\" stroke=\"#ff9900\" stroke-width=\"4\" opacity=\"0\">{}</rect>\n",
            x + 1.0,
            y + 1.0,
            CELL - 2.0,
            CELL - 2.0,
            op.svg("opacity", total)
        ));
    }

    // Eval bar to the right: green's share, from the bottom.
    let bx = PAD + board + 22.0;
    let (by, bh) = (PAD + 2.0, board - 4.0);
    out.push_str(&format!("<rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"14\" height=\"{bh:.0}\" rx=\"7\" fill=\"{RED}\"/>\n"));
    let mut h = Track::new(&format!("{:.1}", bh / 2.0));
    let mut y = Track::new(&format!("{:.1}", by + bh / 2.0));
    for (i, e) in green_eval.iter().enumerate().skip(1) {
        let share = (0.5 + 0.5 * e.clamp(-1.0, 1.0)) * bh;
        let t0 = (i - 1) as f64 * PLY + 0.6;
        h.to(t0, 0.35, &format!("{share:.1}"));
        y.to(t0, 0.35, &format!("{:.1}", by + bh - share));
    }
    h.to(reset, 0.6, &format!("{:.1}", bh / 2.0));
    y.to(reset, 0.6, &format!("{:.1}", by + bh / 2.0));
    out.push_str(&format!(
        "<rect x=\"{bx:.0}\" y=\"{:.1}\" width=\"14\" height=\"{:.1}\" rx=\"7\" fill=\"{GREEN}\">{}{}</rect>\n",
        by + bh / 2.0,
        bh / 2.0,
        h.svg("height", total),
        y.svg("y", total)
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
        let mut op = Track::new("0");
        op.to(i as f64 * PLY + 0.5, 0.05, "1");
        let off = if ply == n { reset } else { (i + 1) as f64 * PLY + 0.5 };
        op.to(off, 0.05, "0");
        out.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{cap_y:.0}\" fill=\"#e6e6e6\" font-size=\"14\" text-anchor=\"middle\" opacity=\"0\">{caption}{}</text>\n",
            PAD + board / 2.0,
            op.svg("opacity", total)
        ));
    }
    out.push_str("</svg>\n");
    print!("{out}");
}
