//! Board diagrams for the docs, as SVG (GitHub renders SVG files in Markdown).
//!
//!     cargo run --release --example diagram -- --green A1,B1 --red C1 [--mark D4,E5] [--last B1] [--label "text"] > docs/img/x.svg
//!
//! Or from a move list, showing the position after those moves with the
//! last move outlined:
//!
//!     cargo run --release --example diagram -- --moves "1. A1 C3 2. B2" > docs/img/y.svg
//!
//! `--numbers A1:1,E1:2` writes numbers on squares (move order).
//! Marks are drawn as hollow circles (used for "the winning squares",
//! "the threat", and so on).

use complete_the_square_ai::game::{play_move, tables, State};

fn square(name: &str) -> usize {
    let b = name.trim().as_bytes();
    let col = (b[0].to_ascii_uppercase() - b'A') as usize;
    let row = (b[1] - b'1') as usize;
    row * 5 + col
}

fn list(arg: Option<&String>) -> Vec<usize> {
    arg.map(|s| s.split(',').filter(|t| !t.trim().is_empty()).map(square).collect()).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));

    let (green, red, mut last): (Vec<usize>, Vec<usize>, Vec<usize>) = if let Some(moves) = get("--moves") {
        let t = tables();
        let mut state = State::new();
        let mut last = Vec::new();
        for token in moves.split_whitespace().filter(|w| !w.ends_with('.')) {
            let sq = square(token);
            state = play_move(t, 1 << sq, &state).0;
            last = vec![sq];
        }
        let green = (0..25).filter(|i| state.boards[0] & (1 << i) != 0).collect();
        let red = (0..25).filter(|i| state.boards[1] & (1 << i) != 0).collect();
        (green, red, last)
    } else {
        (list(get("--green")), list(get("--red")), Vec::new())
    };
    if let Some(l) = get("--last") {
        last = list(Some(l));
    }
    let marks = list(get("--mark"));
    // --numbers A1:1,E1:2 writes a number on each listed square.
    let numbers: Vec<(usize, String)> = get("--numbers")
        .map(|arg| arg.split(',').filter_map(|t| t.split_once(':')).map(|(sq, n)| (square(sq), n.to_string())).collect())
        .unwrap_or_default();
    let label = get("--label").cloned().unwrap_or_default();

    let cell = 40;
    let pad = 24;
    let size = pad + 5 * cell + 8;
    let height = size + if label.is_empty() { 0 } else { 22 };
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{height}\" viewBox=\"0 0 {size} {height}\" font-family=\"system-ui, sans-serif\" font-size=\"13\">\n"
    ));
    out.push_str(&format!("<rect width=\"{size}\" height=\"{height}\" rx=\"6\" fill=\"#121212\"/>\n"));
    for c in 0..5 {
        let x = pad + c * cell + cell / 2;
        out.push_str(&format!("<text x=\"{x}\" y=\"16\" fill=\"#8a8a8a\" text-anchor=\"middle\">{}</text>\n", (b'A' + c as u8) as char));
    }
    for r in 0..5 {
        let y = pad + r * cell + cell / 2 + 5;
        out.push_str(&format!("<text x=\"11\" y=\"{y}\" fill=\"#8a8a8a\" text-anchor=\"middle\">{}</text>\n", r + 1));
        for c in 0..5 {
            let i = r * 5 + c;
            let (x, y) = (pad + c * cell, pad + r * cell);
            let fill = if green.contains(&i) { "#4CAF50" } else if red.contains(&i) { "#F44336" } else { "#2a2a2a" };
            let stroke = if last.contains(&i) { "#ffffff" } else { "#3a3a3a" };
            let width = if last.contains(&i) { 3 } else { 1 };
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{width}\"/>\n",
                x + 2, y + 2, cell - 4, cell - 4
            ));
            if let Some((_, n)) = numbers.iter().find(|(sq, _)| *sq == i) {
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" fill=\"#e6e6e6\" font-size=\"15\" font-weight=\"700\" text-anchor=\"middle\">{n}</text>\n",
                    x + cell / 2, y + cell / 2 + 5
                ));
            }
            if marks.contains(&i) {
                out.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"#ff9900\" stroke-width=\"3\"/>\n",
                    x + cell / 2, y + cell / 2, cell / 2 - 9
                ));
            }
        }
    }
    if !label.is_empty() {
        out.push_str(&format!("<text x=\"{}\" y=\"{}\" fill=\"#cfcfcf\" text-anchor=\"middle\">{label}</text>\n", size / 2, size + 12));
    }
    out.push_str("</svg>\n");
    print!("{out}");
}
