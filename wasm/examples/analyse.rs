//! Replay a game and show, for every move, what the engine would have played.
//!
//!     cargo run --release --example analyse -- "C3 A1 B2 D4 ..." [depth]
//!
//! Moves use the page's notation (column letter, row number from the top).
//! Paste the text from the page's "Copy moves" button; move numbers are ignored.

use complete_the_square_ai::game::{play_move, tables, State};
use complete_the_square_ai::search::{best_move, principal_variation, root_scores};

fn parse(text: &str) -> Vec<u32> {
    text.split_whitespace()
        .filter(|w| !w.ends_with('.'))
        .map(|w| {
            let b = w.as_bytes();
            let col = (b[0].to_ascii_uppercase() - b'A') as usize;
            let row = (b[1] - b'1') as usize;
            1 << (row * 5 + col)
        })
        .collect()
}

fn name(bit: u32) -> String {
    let i = bit.trailing_zeros() as usize;
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}

fn show(state: &State) -> String {
    (0..5)
        .map(|r| {
            (0..5)
                .map(|c| {
                    let b = 1 << (r * 5 + c);
                    if state.boards[0] & b != 0 { 'G' } else if state.boards[1] & b != 0 { 'R' } else { '.' }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let moves = parse(args.get(1).expect("pass the move list as the first argument"));
    let depth: u32 = args.get(2).map(|d| d.parse().unwrap()).unwrap_or(7);
    let t = tables();
    let mut state = State::new();
    let mut history = vec![state.key()];
    let mut game_over = false;

    println!("ply  side  played  engine  (depth {depth})");
    for (i, &bit) in moves.iter().enumerate() {
        let side = if state.current == 0 { "G" } else { "R" };
        let engine = best_move(&state, depth, &history).map(name).unwrap_or_default();
        let flag = if engine != name(bit) { "  <- differs" } else { "" };
        println!("{:>3}  {side}     {:<6}  {:<6}{flag}", i + 1, name(bit), engine);
        let (next, won) = play_move(t, bit, &state);
        state = next;
        history.push(state.key());
        if won.is_some() {
            println!("{side} wins.");
            game_over = true;
            break;
        }
    }
    println!("{}", show(&state));
    if state.empty() != 0 && !game_over {
        let side = if state.current == 0 { "G" } else { "R" };
        println!("\n{side} to move; every move scored at depth {depth} (positive is good for {side}):");
        for (bit, score) in root_scores(&state, depth, &history) {
            println!("  {}  {score:+.3}", name(bit));
        }
        let line: Vec<String> = principal_variation(&state, depth, &history).into_iter().map(name).collect();
        println!("expected line: {}", line.join(" "));
    }
}
