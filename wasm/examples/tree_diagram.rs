//! Draws a two-ply search tree as SVG, a mini board at every node, for the
//! alpha-beta page of the docs. Green is to move at the root; the leaves are
//! scored by the static evaluation (for green). The children are searched in
//! the engine's move order with alpha-beta in minimax form: green takes the
//! maximum over its moves, red the minimum over its replies, and a reply that
//! makes a move no better than the best move found so far cuts off the rest.
//!
//!     cargo run --release --example tree_diagram -- --green A1,C1 --red B1 > docs/img/tree.svg
//!
//! `--full` searches every leaf (plain minimax, no cut-offs).
//! A walk-through of the search is printed to stderr.

use complete_the_square_ai::game::{play_move, tables, State};
use complete_the_square_ai::search::evaluator;

fn square(name: &str) -> usize {
    let b = name.trim().as_bytes();
    (b[1] - b'1') as usize * 5 + (b[0].to_ascii_uppercase() - b'A') as usize
}
fn name(i: usize) -> String {
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}
fn bits(arg: &str) -> u32 {
    arg.split(',').filter(|t| !t.is_empty()).fold(0, |b, t| b | 1 << square(t))
}
fn fmt(x: f64) -> String {
    format!("{x:+.3}")
}

/// A mini board at (x, y), `cell` pixels per square; `outline` squares get a white border.
fn board(out: &mut String, x: f64, y: f64, cell: f64, s: &State, outline: u32, opacity: f64) {
    out.push_str(&format!("<g opacity=\"{opacity}\">\n"));
    for i in 0..25 {
        let (r, c) = ((i / 5) as f64, (i % 5) as f64);
        let fill = if s.boards[0] & (1 << i) != 0 {
            "#4CAF50"
        } else if s.boards[1] & (1 << i) != 0 {
            "#F44336"
        } else {
            "#2a2a2a"
        };
        let (stroke, w) = if outline & (1 << i) != 0 { ("#ffffff", 1.6) } else { ("#121212", 0.6) };
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{cell:.1}\" height=\"{cell:.1}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{w}\"/>\n",
            x + c * cell,
            y + r * cell
        ));
    }
    out.push_str("</g>\n");
}

fn text(out: &mut String, x: f64, y: f64, s: &str, color: &str, size: u32, weight: &str) {
    out.push_str(&format!(
        "<text x=\"{x:.1}\" y=\"{y:.1}\" fill=\"{color}\" font-size=\"{size}\" font-weight=\"{weight}\" text-anchor=\"middle\">{s}</text>\n"
    ));
}

fn line(out: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, width: f64, dashed: bool) {
    let dash = if dashed { " stroke-dasharray=\"4 4\"" } else { "" };
    out.push_str(&format!(
        "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{color}\" stroke-width=\"{width}\"{dash}/>\n"
    ));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let (t, ev) = (tables(), evaluator());
    let root = State { boards: [bits(&get("--green")), bits(&get("--red"))], current: 0 };
    // --full: no pruning (plain minimax), for the negamax page.
    let full = args.iter().any(|a| a == "--full");

    // The tree, in the engine's move order, with the alpha-beta trace.
    struct Leaf {
        state: State,
        reply: u32,
        score: f64,
        visited: bool,
    }
    struct Child {
        state: State,
        mv: u32,
        leaves: Vec<Leaf>,
        value: f64,
        cut: bool,
        alpha_after: f64,
    }
    let order = |s: &State| ev.move_order.iter().copied().filter(|b| b & s.empty() != 0).collect::<Vec<u32>>();
    let mut alpha = f64::NEG_INFINITY;
    let mut children = Vec::new();
    eprintln!("| green's move | red's replies, in order (score for green) | value of the move | alpha after |");
    eprintln!("|---|---|---|---|");
    for mv in order(&root) {
        let (state, _) = play_move(t, mv, &root);
        let mut leaves = Vec::new();
        let mut value = f64::INFINITY;
        let mut cut = false;
        for reply in order(&state) {
            let (leaf, won) = play_move(t, reply, &state);
            let score = if won.is_some() { -1.0 } else { ev.evaluate(t, &leaf) };
            leaves.push(Leaf { state: leaf, reply, score, visited: !cut });
            if !cut {
                value = value.min(score);
                if value <= alpha && !full {
                    cut = true;
                }
            }
        }
        let seen: Vec<String> = leaves.iter().map(|l| {
            let s = format!("{} {}", name(l.reply.trailing_zeros() as usize), fmt(l.score));
            if l.visited { s } else { format!("~~{s}~~") }
        }).collect();
        if value > alpha {
            alpha = value;
        }
        eprintln!(
            "| {} | {} | {}{} | {} |",
            name(mv.trailing_zeros() as usize),
            seen.join(", "),
            if cut { "at most " } else { "" },
            fmt(value),
            fmt(alpha)
        );
        children.push(Child { state, mv, leaves, value, cut, alpha_after: alpha });
    }
    let best = children.iter().filter(|c| !c.cut).max_by(|a, b| a.value.partial_cmp(&b.value).unwrap()).map(|c| c.mv);

    // Layout.
    let leaf_w = 66.0;
    let n_leaves: usize = children.iter().map(|c| c.leaves.len()).sum();
    let gap = 26.0;
    let width = 40.0 + n_leaves as f64 * leaf_w + gap * (children.len() as f64 - 1.0);
    let (root_cell, child_cell, leaf_cell) = (11.0, 8.0, 7.0);
    let (root_y, child_y, leaf_y) = (36.0, 170.0, 330.0);
    let height = leaf_y + 5.0 * leaf_cell + 70.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" font-family=\"system-ui, sans-serif\">\n<rect width=\"100%\" height=\"100%\" rx=\"8\" fill=\"#121212\"/>\n"
    );
    let root_x = width / 2.0 - 2.5 * root_cell;
    text(&mut out, width / 2.0, 22.0, "green to move: takes the maximum", "#cfcfcf", 13, "600");

    let mut x = 20.0;
    for c in &children {
        let span = c.leaves.len() as f64 * leaf_w;
        let cx = x + span / 2.0;
        let is_best = Some(c.mv) == best;
        let (edge, ew) = if is_best { ("#4CAF50", 3.0) } else { ("#555555", 1.5) };
        line(&mut out, width / 2.0, root_y + 5.0 * root_cell, cx, child_y - 4.0, edge, ew, false);
        board(&mut out, cx - 2.5 * child_cell, child_y, child_cell, &c.state, c.mv, 1.0);
        let label = format!("green {}", name(c.mv.trailing_zeros() as usize));
        text(&mut out, cx, child_y + 5.0 * child_cell + 16.0, &label, "#e6e6e6", 13, "700");
        let value = if c.cut { format!("≤ {} (cut off)", fmt(c.value)) } else { format!("= {}", fmt(c.value)) };
        text(&mut out, cx, child_y + 5.0 * child_cell + 32.0, &value, if is_best { "#8ce68f" } else { "#cfcfcf" }, 12, "400");
        text(&mut out, cx, child_y + 5.0 * child_cell + 48.0, &format!("α = {}", fmt(c.alpha_after)), "#ff9900", 12, "400");
        text(&mut out, cx, leaf_y - 34.0, "red: minimum", "#8a8a8a", 11, "400");

        for (j, l) in c.leaves.iter().enumerate() {
            let lx = x + j as f64 * leaf_w + leaf_w / 2.0;
            let opacity = if l.visited { 1.0 } else { 0.28 };
            line(&mut out, cx, child_y + 5.0 * child_cell + 54.0, lx, leaf_y - 4.0, if l.visited { "#555555" } else { "#3a3a3a" }, 1.2, !l.visited);
            board(&mut out, lx - 2.5 * leaf_cell, leaf_y, leaf_cell, &l.state, l.reply, opacity);
            let rl = format!("red {}", name(l.reply.trailing_zeros() as usize));
            text(&mut out, lx, leaf_y + 5.0 * leaf_cell + 15.0, &rl, if l.visited { "#cfcfcf" } else { "#555555" }, 11, "400");
            let sl = if l.visited { fmt(l.score) } else { "pruned".to_string() };
            text(&mut out, lx, leaf_y + 5.0 * leaf_cell + 30.0, &sl, if l.visited { "#e6e6e6" } else { "#555555" }, 12, if l.visited { "700" } else { "400" });
        }
        x += span + gap;
    }
    board(&mut out, root_x, root_y, root_cell, &root, 0, 1.0);
    out.push_str("</svg>\n");
    print!("{out}");
}
