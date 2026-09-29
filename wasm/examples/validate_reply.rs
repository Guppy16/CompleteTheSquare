//! Validate opening-book candidates by play: after a forced first move, play
//! each candidate reply and let the arena's players continue (human-like
//! green with random blunders vs red at the page budget). Depth-14 scores
//! can be wrong in play (see the C3 entry); this is the check that catches it.
//!
//!     cargo run --release --example validate_reply -- C3 A1 C5 C1 [games]
//!
//! prints red's record after 1. C3 with each of the replies A1, C5, C1.

use complete_the_square_ai::game::{play_move, tables, State, N};
use complete_the_square_ai::search::{best_move, best_move_scored, TranspositionTable};

const NODE_BUDGET: u64 = 400_000; // same as the page
const BLUNDER_PCT: usize = 10;

fn square(name: &str) -> u32 {
    let b = name.as_bytes();
    let col = (b[0].to_ascii_uppercase() - b'A') as usize;
    let row = (b[1] - b'1') as usize;
    1 << (row * 5 + col)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: validate_reply FIRST REPLY [REPLY...] [games]");
        return;
    }
    let first = square(&args[1]);
    let (replies, games): (Vec<&String>, usize) = match args.last().unwrap().parse::<usize>() {
        Ok(n) => (args[2..args.len() - 1].iter().collect(), n),
        Err(_) => (args[2..].iter().collect(), 40),
    };
    let t = tables();
    let (after_first, _) = play_move(t, first, &State::new());

    for reply in replies {
        let mut seed: u32 = 777;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as usize
        };
        let (mut red_wins, mut green_wins, mut draws) = (0, 0, 0);
        for _ in 0..games {
            let tt = TranspositionTable::new();
            let (mut state, _) = play_move(t, square(reply), &after_first);
            let mut history = vec![State::new().key(), after_first.key(), state.key()];
            let mut plies = 2;
            let result = loop {
                let bit = if state.current == 0 {
                    let empty = state.empty();
                    if next() % 100 < BLUNDER_PCT {
                        let legal: Vec<u32> = (0..N).map(|i| 1 << i).filter(|b| b & empty != 0).collect();
                        legal[next() % legal.len()]
                    } else {
                        best_move(&state, 5, &history).unwrap()
                    }
                } else {
                    best_move_scored(&state, 12, NODE_BUDGET, &history, &tt, false).0.unwrap().0
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
            match result {
                0 => green_wins += 1,
                1 => red_wins += 1,
                _ => draws += 1,
            }
        }
        println!("1. {} {reply}: red {red_wins} / green {green_wins} / draws {draws} of {games}", args[1].to_uppercase());
    }
}
