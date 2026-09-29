//! Timing of the parallel search: scores every reply to 1. A1 at the given
//! depth with each thread count, and the single best move by lazy SMP.
//!
//!     cargo run --release --example bench -- [depth] [thread counts...]
//!     cargo run --release --example bench -- 11 1 4 8 16

use complete_the_square_ai::game::{play_move, square_bit, tables, State};
use complete_the_square_ai::parallel::{best_move_parallel, root_scores_parallel};
use std::time::Instant;

fn name(b: u32) -> String {
    let i = b.trailing_zeros() as usize;
    format!("{}{}", (b'A' + (i % 5) as u8) as char, i / 5 + 1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let depth: u32 = args.get(1).map(|d| d.parse().unwrap()).unwrap_or(10);
    let counts: Vec<usize> = if args.len() > 2 { args[2..].iter().map(|n| n.parse().unwrap()).collect() } else { vec![1, 2, 4, 8] };
    let (state, _) = play_move(tables(), square_bit(0, 0), &State::new());
    let history = [State::new().key(), state.key()];

    println!("position: after 1. A1, red to move, depth {depth}");
    for &threads in &counts {
        let start = Instant::now();
        let scores = root_scores_parallel(&state, depth, &history, threads);
        let t_scores = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let (best, reached) = best_move_parallel(&state, depth, &history, threads);
        let t_best = start.elapsed().as_secs_f64();
        let top: Vec<String> = scores.iter().take(3).map(|(b, s)| format!("{} {s:+.3}", name(*b))).collect();
        println!(
            "threads {threads:>2}: root scores {t_scores:>7.2} s [{}]   best move {t_best:>7.2} s ({} at depth {reached})",
            top.join(", "),
            best.map(|(b, _)| name(b)).unwrap_or_default()
        );
    }
}
