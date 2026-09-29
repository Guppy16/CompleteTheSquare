//! Does quiescence search pay for itself? Plays the arena's games (the AI
//! as red against a depth-5 opponent that blunders 10% of the time) twice:
//! once with quiescence (4 plies of captures at every leaf) and once
//! without (leaves take the static evaluation). Both at a fixed depth and
//! at the page's node budget, since quiescence costs nodes.
//!
//!     cargo run --release --example experiment_quiescence -- [games]

use complete_the_square_ai::game::{play_move, tables, State, N};
use complete_the_square_ai::search::{best_move, best_move_scored, TranspositionTable, QUIESCENCE_OVERRIDE};
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Red's move chooser: fixed depth (budget = u64::MAX) or the page's budget.
fn play(games: usize, depth: u32, budget: u64) -> (usize, usize, u64, f64) {
    let t = tables();
    let mut seed: u32 = 777;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as usize
    };
    let (mut red_wins, mut red_moves, mut red_nodes) = (0, 0u64, 0u64);
    let mut depth_sum = 0u64;
    for _ in 0..games {
        let tt = TranspositionTable::new();
        let mut state = State::new();
        let mut history = vec![state.key()];
        let mut plies = 0;
        loop {
            let bit = if state.current == 0 {
                // The opponent always searches with quiescence on.
                let saved = QUIESCENCE_OVERRIDE.swap(u32::MAX, Ordering::Relaxed);
                let empty = state.empty();
                let m = if next() % 100 < 10 {
                    let legal: Vec<u32> = (0..N).map(|i| 1 << i).filter(|b| b & empty != 0).collect();
                    legal[next() % legal.len()]
                } else {
                    best_move(&state, 5, &history).unwrap()
                };
                QUIESCENCE_OVERRIDE.store(saved, Ordering::Relaxed);
                m
            } else {
                let (best, reached, nodes) = best_move_scored(&state, depth, budget, &history, &tt, true);
                red_moves += 1;
                red_nodes += nodes;
                depth_sum += reached as u64;
                best.unwrap().0
            };
            let (s, won) = play_move(t, bit, &state);
            state = s;
            history.push(state.key());
            plies += 1;
            if won.is_some() {
                if state.current == 1 {
                    red_wins += 1;
                }
                break;
            }
            if history.iter().filter(|&&k| k == state.key()).count() >= 3 || plies > 200 {
                break;
            }
        }
    }
    (red_wins, games, red_nodes / red_moves.max(1), depth_sum as f64 / red_moves.max(1) as f64)
}

fn main() {
    let games: usize = std::env::args().nth(1).map(|g| g.parse().unwrap()).unwrap_or(100);
    println!("AI as red vs a depth-5 opponent with 10% blunders, {games} games each");
    for (label, depth, budget) in [("fixed depth 5", 5, u64::MAX), ("page budget, 400k nodes", 12, 400_000)] {
        for (q, qname) in [(0u32, "without quiescence"), (u32::MAX, "with quiescence   ")] {
            QUIESCENCE_OVERRIDE.store(q, Ordering::Relaxed);
            let start = Instant::now();
            let (wins, n, nodes, avg_depth) = play(games, depth, budget);
            println!(
                "{label:<24} {qname}: red won {wins:>3} of {n}   ({nodes:>7} nodes/move, depth {avg_depth:.1}, {:.0}s)",
                start.elapsed().as_secs_f64()
            );
        }
    }
    QUIESCENCE_OVERRIDE.store(u32::MAX, Ordering::Relaxed);
}
