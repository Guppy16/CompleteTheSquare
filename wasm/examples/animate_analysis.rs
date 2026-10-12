//! An animated SVG of the page's Analysis tab, for the README. From the empty
//! board, a cursor plays the first suggested move a few times (on its square,
//! or a book move by clicking its line in the panel); for every
//! position the engine's top lines appear as on the page: the panel above the
//! board (depth, each line's score and moves, the book move first), a marker
//! on each candidate square holding a mini board of where its line ends, with
//! a glow in the mover's colour and a small eval bar, and the eval bar beside
//! the board springing to the best score. Loops.
//!
//! The analysis is the engine's own (`analyse_position` with the page's final
//! budget), so the picture is what the page shows once it has finished thinking.
//!
//!     cargo run --release --example animate_analysis -- 3 > docs/img/analysis.svg

use complete_the_square_ai::game::{play_move, tables, State};
use complete_the_square_ai::search::{analyse_position, book_move, TranspositionTable, WIN_DEPTH_BONUS, WIN_SCORE};

#[path = "common/anim.rs"]
mod anim;
use anim::*;

// The page's analysis (script.js).
const MAX_DEPTH: u32 = 12;
const BUDGET: u64 = 8_000_000;
const LINES: usize = 3;
const GROW: f64 = 0.4 * MOTION; // a marker growing in
const GROW_STEP: f64 = 0.08 * MOTION; // between markers, best first
const BAR: f64 = 0.6 * MOTION; // the eval bar springing to a new score

// The cursor's pace (not on the page).
const START: f64 = 0.5; // before the first analysis
const THINK: f64 = 1.8; // reading the analysis before moving
const TRAVEL: f64 = 0.4; // gliding to the chosen square
const AIM: f64 = 0.3; // hovering it before the click
const ANALYSE: f64 = 0.4; // after the click, before the next analysis shows
const HOLD: f64 = 2.5; // on the final analysis
const RESET: f64 = 0.6; // fading back to the empty board

// Layout: the panel, then the board with the eval column on its right.
const PANEL_H: f64 = 84.0;
const BOARD_Y: f64 = PANEL_H + 14.0; // the board's group starts here
const PANEL_X: f64 = 10.0;
const EVAL_GAP: f64 = 12.0;
const EVAL_W: f64 = 14.0;
const LIGHT_GREEN: &str = "#8ce68f";
const LIGHT_RED: &str = "#ff8a80";

/// A position's analysis as the page shows it.
struct Analysis {
    depth: u32,
    best: f64,                            // the best score, for Green
    top: f64,                             // the best score, for the side to move
    lines: Vec<(usize, f64, Vec<usize>)>, // square, score for the side to move, the line
    book: Option<usize>,
    toward: usize,                      // the mover: 0 Green, 1 Red
}

fn analyse(state: &State, history: &[u64]) -> Analysis {
    let tt = TranspositionTable::new();
    let (candidates, depth) = analyse_position(state, MAX_DEPTH, BUDGET, history, &tt);
    let for_green = |s: f64| if state.current == 0 { s } else { -s };
    let book = book_move(state).map(|b| b.trailing_zeros() as usize);
    let all: Vec<(usize, f64, Vec<usize>)> = candidates
        .iter()
        .map(|c| (c.bit.trailing_zeros() as usize, c.score, c.line.iter().map(|b| b.trailing_zeros() as usize).collect()))
        .collect();
    let best = for_green(all.first().expect("a position with moves").1);
    // As the page: the top lines, the book move first if there is one.
    let mut shown: Vec<_> = all.iter().take(LINES).cloned().collect();
    if let Some(b) = book {
        let line = all.iter().find(|l| l.0 == b).expect("the book move is legal").clone();
        shown.retain(|l| l.0 != b);
        shown.insert(0, line);
        shown.truncate(LINES);
    }
    Analysis { depth, best, top: all[0].1, lines: shown, book, toward: state.current }
}

/// The baseline of the panel's `k`-th line.
fn row_y(k: usize) -> f64 {
    30.0 + 20.0 * (k as f64 + 1.0)
}

/// A score for Green as the page writes it: +0.08, or #3 for a forced win.
fn score_text(green: f64, depth: u32) -> String {
    if green.abs() < WIN_SCORE {
        return format!("{}{:.2}", if green >= 0.0 { "+" } else { "" }, green);
    }
    let plies = (depth as i64 - ((green.abs() - 1.0) / WIN_DEPTH_BONUS).round() as i64 + 1).max(1);
    format!("#{}{}", if green < 0.0 { "-" } else { "" }, (plies + 1) / 2)
}

/// A line as lichess writes it, `ply` moves into the game.
fn line_text(squares: &[usize], ply: usize) -> String {
    let mut parts = Vec::new();
    for (i, &sq) in squares.iter().enumerate() {
        let p = ply + i;
        if p % 2 == 0 {
            parts.push(format!("{}.", p / 2 + 1));
        } else if i == 0 {
            parts.push(format!("{}...", p / 2 + 1));
        }
        parts.push(name(sq));
    }
    parts.join(" ")
}

/// The line's text cut to `max` characters at a move, with an ellipsis.
fn fit(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let mut words: Vec<&str> = Vec::new();
    for word in text.split(' ') {
        if words.iter().map(|w| w.len() + 1).sum::<usize>() + word.len() + 2 > max {
            break;
        }
        words.push(word);
    }
    if words.last().is_some_and(|w| w.ends_with('.')) {
        words.pop(); // a move number with no move after it
    }
    words.join(" ") + " …"
}

/// Green's share of an eval bar, from 0 to 1.
fn share(green: f64) -> f64 {
    0.5 + 0.5 * green.clamp(-1.0, 1.0)
}

/// Shown from `on` to `off`: an opacity animation.
fn shown(on: f64, off: f64, total: f64) -> String {
    let mut op = Track::new(0.0);
    op.set(on, 1.0);
    op.set(off, 0.0);
    op.svg("opacity", total)
}

fn main() {
    let plies: usize = std::env::args().nth(1).map_or(3, |a| a.parse().expect("a number of moves"));
    let t = tables();

    // The positions and their analyses; the cursor plays the first line shown.
    let mut states = vec![State::new()];
    let mut history = vec![State::new().key()];
    let mut analyses = Vec::new();
    let mut moves = Vec::new();
    for i in 0..=plies {
        let state = *states.last().unwrap();
        let a = analyse(&state, &history);
        eprintln!("{}: depth {} best {:+.3} lines {:?}", i, a.depth, a.best, a.lines.iter().map(|l| name(l.0)).collect::<Vec<_>>());
        if i < plies {
            let m = a.lines[0].0;
            let (next, won) = play_move(t, 1 << m, &state);
            assert!(won.is_none(), "the animation stops before a win");
            moves.push(m);
            states.push(next);
            history.push(next.key());
        }
        analyses.push(a);
    }

    // Timeline: each analysis shows at `shows[i]`, until the click at `clicks[i]`.
    let mut shows = Vec::new();
    let mut clicks = Vec::new();
    let mut now = START;
    let (mut cursor_x, mut cursor_y) = (Track::new(PAD + 5.0 * CELL - 8.0), Track::new(PAD + 5.0 * CELL + 14.0));
    let rest = (cursor_x.last(), cursor_y.last());
    let (mut frame_x, mut frame_y, mut frame_op) = (Track::new(0.0), Track::new(0.0), Track::new(0.0));
    let mut colour: Vec<Colour> = (0..25).map(|_| Colour::new(EMPTY)).collect();
    let mut scale: Vec<Track> = (0..25).map(|_| Track::new(1.0)).collect();
    let mut bar = Track::new(0.5);
    let mut row_hover = Vec::new(); // (analysis, from, to): the top line highlighted under the cursor
    for i in 0..=plies {
        shows.push(now);
        bar.spring(now, BAR, bar.last(), share(analyses[i].best));
        now += THINK;
        if i == plies {
            break;
        }
        let m = moves[i];
        // A book move is played from the panel: a click on its line (the top
        // one) plays its first move, as on the page. Others are played on the board.
        let from_panel = analyses[i].book.is_some();
        let (x, y) = if from_panel { (PANEL_X + 150.0, row_y(0) - 4.0 - BOARD_Y) } else { centre(m) };
        cursor_x.ease(now, TRAVEL, x + 4.0);
        cursor_y.ease(now, TRAVEL, y + 4.0);
        let arrive = now + TRAVEL;
        let click = arrive + AIM;
        if from_panel {
            row_hover.push((i, arrive, click));
        } else {
            frame_x.set(arrive, x - SQUARE / 2.0);
            frame_y.set(arrive, y - SQUARE / 2.0);
            frame_op.keys(arrive, FRAME_FADE, &[(1.0, 0.7)]);
            frame_op.keys(click, FRAME_FADE, &[(1.0, 0.0)]);
        }
        clicks.push(click);
        let (before, after) = (states[i], states[i + 1]);
        let mover = if before.current == 0 { GREEN } else { RED };
        for c in 0..25 {
            let bit = 1u32 << c;
            if before.occupied() & bit == 0 && after.occupied() & bit != 0 {
                colour[c].to(click, 0.0, mover);
                scale[c].spring(click, DROP, 0.5, 1.0);
            } else if before.occupied() & bit != 0 && after.occupied() & bit == 0 {
                let from = scale[c].last();
                colour[c].to(click + 0.3 * FADE, 0.7 * FADE, EMPTY);
                scale[c].keys(click, FADE, &[(0.0, from), (0.3, 1.12), (1.0, 1.0)]);
            }
        }
        now = click + ANALYSE;
    }
    let reset = now + HOLD - THINK;
    clicks.push(reset); // the last analysis hides at the reset
    for c in colour.iter_mut() {
        c.to(reset, RESET, EMPTY);
    }
    bar.keys(reset, RESET, &[(1.0, 0.5)]);
    cursor_x.ease(reset, RESET, rest.0);
    cursor_y.ease(reset, RESET, rest.1);
    let total = reset + RESET + 0.2;

    let board = 5.0 * CELL;
    let width = PAD + board + EVAL_GAP + EVAL_W + PAD;
    let height = BOARD_Y + PAD + board + 12.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" font-family=\"system-ui, sans-serif\">\n<rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"#121212\"/>\n"
    );
    out.push_str("<defs><filter id=\"glow-0\" x=\"-30%\" y=\"-30%\" width=\"160%\" height=\"160%\"><feDropShadow dx=\"0\" dy=\"0\" stdDeviation=\"2\" flood-color=\"#4CAF50\"/></filter><filter id=\"glow-1\" x=\"-30%\" y=\"-30%\" width=\"160%\" height=\"160%\"><feDropShadow dx=\"0\" dy=\"0\" stdDeviation=\"2\" flood-color=\"#F44336\"/></filter></defs>\n");

    // The panel: depth, then the lines, as on the page.
    let (px, pw) = (PANEL_X, width - 2.0 * PANEL_X);
    out.push_str(&format!("<rect x=\"{px}\" y=\"10\" width=\"{pw:.0}\" height=\"{PANEL_H:.0}\" rx=\"4\" fill=\"#1e1e1e\"/>\n"));
    for (i, a) in analyses.iter().enumerate() {
        let vis = shown(shows[i], clicks[i], total);
        out.push_str(&format!(
            "<g opacity=\"0\">{vis}<text x=\"{:.0}\" y=\"28\" fill=\"#8a8a8a\" font-size=\"12\">Depth {}</text>\n",
            px + 8.0,
            a.depth
        ));
        for &(_, from, to) in row_hover.iter().filter(|h| h.0 == i) {
            out.push_str(&format!(
                "<rect x=\"{px}\" y=\"{:.0}\" width=\"{pw:.0}\" height=\"20\" fill=\"#2a2a2a\" opacity=\"0\">{}</rect>\n",
                row_y(0) - 14.0,
                shown(from, to, total)
            ));
        }
        for (k, (_, score, line)) in a.lines.iter().enumerate() {
            let y = row_y(k);
            let green = if a.toward == 0 { *score } else { -score };
            let is_book = a.book == Some(line[0]);
            let max = if is_book { 28 } else { 34 };
            out.push_str(&format!(
                "<line x1=\"{px}\" x2=\"{:.0}\" y1=\"{:.0}\" y2=\"{:.0}\" stroke=\"#3a3a3a\"/><text x=\"{:.0}\" y=\"{y:.0}\" fill=\"#e6e6e6\" font-size=\"12\" font-weight=\"700\">{}</text><text x=\"{:.0}\" y=\"{y:.0}\" fill=\"#e6e6e6\" font-size=\"12\">{}</text>\n",
                px + pw,
                y - 14.0,
                y - 14.0,
                px + 8.0,
                score_text(green, a.depth),
                px + 52.0,
                fit(line_text(line, i), max)
            ));
            if is_book {
                out.push_str(&format!(
                    "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"32\" height=\"15\" rx=\"3\" fill=\"#2a2a2a\"/><text x=\"{:.0}\" y=\"{:.0}\" fill=\"#8a8a8a\" font-size=\"10\" text-anchor=\"middle\">book</text>\n",
                    px + pw - 38.0,
                    y - 11.0,
                    px + pw - 22.0,
                    y
                ));
            }
        }
        out.push_str("</g>\n");
    }

    // The board, below the panel.
    out.push_str(&format!("<g transform=\"translate(0 {BOARD_Y:.0})\">\n"));
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
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{SQUARE:.0}\" height=\"{SQUARE:.0}\" fill=\"{EMPTY}\">{}{}</rect>\n",
            cx - SQUARE / 2.0,
            cy - SQUARE / 2.0,
            smil("fill", &colour[c].0, total),
            scaled(cx, cy, SQUARE, &scale[c], total),
        ));
    }
    // The last move: a soft white inner outline.
    for (i, &m) in moves.iter().enumerate() {
        let (cx, cy) = centre(m);
        let off = if i + 1 < moves.len() { clicks[i + 1] } else { reset };
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"none\" stroke=\"#ffffff\" stroke-opacity=\"0.45\" stroke-width=\"3\" opacity=\"0\">{}</rect>\n",
            cx - SQUARE / 2.0 + 1.5,
            cy - SQUARE / 2.0 + 1.5,
            SQUARE - 3.0,
            SQUARE - 3.0,
            shown(clicks[i], off, total)
        ));
    }

    // The markers: a mini board of where each line ends, sized by the gap to
    // the best move, growing in best first; a small eval bar on the square's edge.
    for (i, a) in analyses.iter().enumerate() {
        for (rank, (sq, score, line)) in a.lines.iter().enumerate() {
            let gap = if a.book == Some(*sq) { 0.0 } else { (a.top - score).abs() };
            let size = (1.0 - gap * 4.0).max(0.55);
            let opacity = (1.0 - gap * 3.0).max(0.45);
            let side = (SQUARE * 0.66 * size).round();
            let (cx, cy) = centre(*sq);
            // The board at the end of the line, and its last two moves.
            let mut end = states[i];
            for &m in line {
                end = play_move(t, 1 << m, &end).0;
            }
            let last: Vec<usize> = line.iter().rev().take(2).copied().collect();
            let cell = (side - 1.0) / 5.0;
            let mut mini = format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{side:.0}\" height=\"{side:.0}\" fill=\"#121212\"{}/>",
                -side / 2.0,
                -side / 2.0,
                if a.book == Some(*sq) { " stroke=\"#ffffff\" stroke-width=\"2\"" } else { "" }
            );
            for c in 0..25 {
                let bit = 1u32 << c;
                let fill = match (end.boards[0] & bit != 0, end.boards[1] & bit != 0, last.contains(&c)) {
                    (true, _, true) => LIGHT_GREEN,
                    (true, _, false) => GREEN,
                    (_, true, true) => LIGHT_RED,
                    (_, true, false) => RED,
                    _ => EMPTY,
                };
                mini.push_str(&format!(
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{fill}\"/>",
                    -side / 2.0 + 1.0 + (c % 5) as f64 * cell,
                    -side / 2.0 + 1.0 + (c / 5) as f64 * cell,
                    cell - 1.0,
                    cell - 1.0
                ));
            }
            let mut k = Track::new(0.5);
            k.spring(shows[i] + GROW_STEP * rank as f64, GROW, 0.5, 1.0);
            let grow = smil("transform", &k.frames(|v| format!("{v:.3}")), total)
                .replace("<animate attributeName=\"transform\"", "<animateTransform attributeName=\"transform\" type=\"scale\"");
            out.push_str(&format!(
                "<g opacity=\"0\">{}<g transform=\"translate({cx:.1} {cy:.1})\" opacity=\"{opacity:.2}\"><g filter=\"url(#glow-{})\">{grow}{mini}</g></g>",
                shown(shows[i], clicks[i], total),
                a.toward
            ));
            let green = if a.toward == 0 { *score } else { -score };
            let h = SQUARE * share(green);
            out.push_str(&format!(
                "<g opacity=\"{opacity:.2}\"><rect x=\"{:.1}\" y=\"{:.1}\" width=\"4\" height=\"{SQUARE:.0}\" fill=\"{RED}\"/><rect x=\"{:.1}\" y=\"{:.1}\" width=\"4\" height=\"{h:.1}\" fill=\"{GREEN}\"/><rect x=\"{:.1}\" y=\"{:.1}\" width=\"4\" height=\"{SQUARE:.0}\" fill=\"none\" stroke=\"#000\" stroke-opacity=\"0.35\"/></g></g>\n",
                cx + SQUARE / 2.0 - 4.0,
                cy - SQUARE / 2.0,
                cx + SQUARE / 2.0 - 4.0,
                cy + SQUARE / 2.0 - h,
                cx + SQUARE / 2.0 - 4.0,
                cy - SQUARE / 2.0,
            ));
        }
    }

    // The eval column: the best score above a bar, green from the bottom.
    let (bx, by) = (PAD + board + EVAL_GAP, PAD + 2.0);
    let bh = board - 4.0;
    let green_y = bar.frames(|s| format!("{:.1}", by + bh * (1.0 - s)));
    let green_h = bar.frames(|s| format!("{:.1}", bh * s));
    out.push_str(&format!(
        "<rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"{EVAL_W:.0}\" height=\"{bh:.0}\" fill=\"{RED}\"/><rect x=\"{bx:.0}\" y=\"{:.1}\" width=\"{EVAL_W:.0}\" height=\"{:.1}\" fill=\"{GREEN}\">{}{}</rect><rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"{EVAL_W:.0}\" height=\"{bh:.0}\" fill=\"none\" stroke=\"#000\" stroke-opacity=\"0.35\"/>\n",
        by + bh / 2.0,
        bh / 2.0,
        smil("y", &green_y, total),
        smil("height", &green_h, total)
    ));
    for (i, a) in analyses.iter().enumerate() {
        out.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.0}\" fill=\"#e6e6e6\" font-size=\"10\" font-weight=\"700\" text-anchor=\"middle\" opacity=\"0\">{}{}</text>\n",
            bx + EVAL_W / 2.0,
            by - 6.0,
            score_text(a.best, a.depth),
            shown(shows[i], clicks[i], total)
        ));
    }

    // The hover frame, then the cursor on top.
    out.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{SQUARE:.0}\" height=\"{SQUARE:.0}\" fill=\"none\" stroke=\"#e6e6e6\" stroke-width=\"2\" opacity=\"0\">{}{}{}</rect>\n",
        frame_x.svg("x", total),
        frame_y.svg("y", total),
        frame_op.svg("opacity", total)
    ));
    let pos: Vec<(f64, String)> =
        cursor_x.0.iter().zip(&cursor_y.0).map(|(&(t, x), &(_, y))| (t, format!("{x:.1} {y:.1}"))).collect();
    let translate = smil("transform", &pos, total)
        .replace("<animate attributeName=\"transform\"", "<animateTransform attributeName=\"transform\" type=\"translate\"");
    out.push_str(&format!(
        "<g transform=\"translate({:.1} {:.1})\"><path d=\"M0 0 L0 17 L4.6 12.6 L7.7 19.4 L10.4 18.2 L7.4 11.6 L13 11.6 Z\" fill=\"#ffffff\" stroke=\"#121212\" stroke-width=\"1.2\" stroke-linejoin=\"round\"/>{translate}</g>\n",
        rest.0, rest.1
    ));
    out.push_str("</g>\n</svg>\n");
    print!("{out}");
}
