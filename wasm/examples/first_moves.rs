//! Score Green's first moves at increasing depths: how much is moving first worth?
//!
//!     cargo run --release --example first_moves -- [max_depth] [threads]
//!
//! With `--features rotated-squares`, for the variant where tilted squares win
//! too. The board's symmetries leave six distinct first moves; scores are for
//! Green, from -1 to +1, and just above +1 is a forced win.

use std::time::Instant;

use complete_the_square_ai::game::State;
use complete_the_square_ai::parallel::root_scores_parallel;

const OPENINGS: [(&str, u32); 6] = [("A1", 0), ("B1", 1), ("C1", 2), ("B2", 6), ("C2", 7), ("C3", 12)];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let max_depth: u32 = args.get(1).map_or(10, |a| a.parse().expect("a depth"));
    let threads: usize = args.get(2).map_or(8, |a| a.parse().expect("a thread count"));
    let rules = if cfg!(feature = "rotated-squares") { "rotated squares" } else { "standard" };
    println!("{rules} rules, {threads} threads");
    println!("depth  {}  seconds", OPENINGS.map(|(name, _)| format!("{name:>6}")).join(" "));

    let empty = State::new();
    for depth in 1..=max_depth {
        let started = Instant::now();
        let scores = root_scores_parallel(&empty, depth, &[], threads);
        let score = |index: u32| scores.iter().find(|(bit, _)| *bit == 1 << index).expect("every move is scored").1;
        let row = OPENINGS.map(|(_, index)| format!("{:>+6.3}", score(index))).join(" ");
        println!("{depth:>5}  {row}  {:>7.1}", started.elapsed().as_secs_f64());
    }
}
