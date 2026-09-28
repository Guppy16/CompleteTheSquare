//! Strength benchmark: the AI as red against a human-like green.
//!
//!     cargo run --release --example arena -- [blunder_percent] [games] [node_budget]
//!
//! Green plays the engine's own depth-5 move, except that `blunder_percent`
//! of the time it plays a random legal move instead. Red is the engine as the
//! page runs it (iterative deepening under a node budget). This is the
//! deployed situation, and the number to improve. Engine-vs-engine matches
//! from short openings are not useful here: the side to move wins over 90%
//! of games at equal strength, so they measure the seat, not the engine.

use complete_the_square_ai::game::{play_move, tables, State, N};
use complete_the_square_ai::search::{best_move, search_depth_reached, TranspositionTable};
use std::time::Instant;

const MAX_DEPTH: u32 = 12;
const NODE_BUDGET: u64 = 400_000; // same as the page; the 3rd argument overrides it

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let blunder_pct: usize = args.get(1).map(|a| a.parse().unwrap()).unwrap_or(10);
    let games: usize = args.get(2).map(|a| a.parse().unwrap()).unwrap_or(200);
    let node_budget: u64 = args.get(3).map(|a| a.parse().unwrap()).unwrap_or(NODE_BUDGET);

    let t = tables();
    let start = Instant::now();
    let mut seed: u32 = 777;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as usize
    };
    let (mut red_wins, mut green_wins, mut draws, mut total_plies) = (0, 0, 0, 0);
    let (mut red_nodes, mut red_moves, mut red_depth) = (0u64, 0u64, 0u64);
    for _ in 0..games {
        let mut tt = TranspositionTable::new(); // persists for the game, as on the page
        let mut state = State::new();
        let mut history = vec![state.key()];
        let mut plies = 0;
        let result = loop {
            let bit = if state.current == 0 {
                let empty = state.empty();
                if next() % 100 < blunder_pct {
                    let legal: Vec<u32> = (0..N).map(|i| 1 << i).filter(|b| b & empty != 0).collect();
                    legal[next() % legal.len()]
                } else {
                    best_move(&state, 5, &history).unwrap()
                }
            } else {
                let (bit, depth, nodes) = search_depth_reached(&state, MAX_DEPTH, node_budget, &history, &mut tt);
                red_depth += depth as u64;
                red_nodes += nodes;
                red_moves += 1;
                bit.unwrap()
            };
            let (s, won) = play_move(t, bit, &state);
            state = s;
            history.push(state.key());
            plies += 1;
            if won.is_some() {
                break state.current as i32;
            }
            if history.iter().filter(|&&k| k == state.key()).count() >= 3 || plies > 200 {
                break -1;
            }
        };
        total_plies += plies;
        match result {
            0 => green_wins += 1,
            1 => red_wins += 1,
            _ => draws += 1,
        }
    }
    println!(
        "AI as red vs human-like green ({blunder_pct}% blunders): red {red_wins} / green {green_wins} / draws {draws} of {games}  (avg plies {:.0}, {:.0}s, {:.0}k nodes and depth {:.1} per AI move)",
        total_plies as f64 / games as f64,
        start.elapsed().as_secs_f64(),
        red_nodes as f64 / red_moves.max(1) as f64 / 1000.0,
        red_depth as f64 / red_moves.max(1) as f64
    );
}
