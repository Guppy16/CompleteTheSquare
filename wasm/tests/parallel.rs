//! The parallel search must agree with the single-threaded one, and the
//! shared table must never hand back a corrupted entry. Each test repeats a
//! few times because races do not show up on every run.

use complete_the_square_ai::game::{play_move, square_bit, tables, State, N};
use complete_the_square_ai::parallel::{best_move_parallel, root_scores_parallel, SharedTable};
use complete_the_square_ai::search::{best_move_scored, root_scores, Bound, Table, TranspositionTable, TtEntry};

/// A tiny deterministic RNG so the tests need no dependencies.
fn xorshift(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

/// A random position a few random moves in, not already won.
fn random_position(seed: &mut u32, max_moves: u32) -> Option<State> {
    let t = tables();
    let mut state = State::new();
    for _ in 0..(xorshift(seed) % max_moves) {
        let empty = state.empty();
        let legal: Vec<u32> = (0..N).map(|i| 1 << i).filter(|b| b & empty != 0).collect();
        let (child, won) = play_move(t, legal[xorshift(seed) as usize % legal.len()], &state);
        if won.is_some() {
            return None;
        }
        state = child;
    }
    Some(state)
}

#[test]
fn shared_table_never_returns_a_corrupt_entry() {
    // Two threads hammer the same handful of slots with different entries.
    // Every read must be either a miss or a self-consistent entry whose key
    // matches: the XOR check turns torn writes into misses.
    let table = SharedTable::new();
    table.bump_age();
    let keys: Vec<u64> = (1..=8u64).map(|k| k * 0x9E37_79B9).collect();
    std::thread::scope(|scope| {
        for thread in 0..2u32 {
            let (table, keys) = (&table, &keys);
            scope.spawn(move || {
                let mut seed = 0x1234_5678 + thread;
                for _ in 0..200_000 {
                    let key = keys[xorshift(&mut seed) as usize % keys.len()];
                    let depth = xorshift(&mut seed) % 12;
                    let square = xorshift(&mut seed) % 25;
                    table.put(TtEntry {
                        key,
                        depth,
                        score: depth as f64 / 16.0,
                        bound: Bound::Exact,
                        best: 1 << square,
                        age: 0,
                    });
                    let key = keys[xorshift(&mut seed) as usize % keys.len()];
                    if let Some(e) = table.get(key) {
                        assert_eq!(e.key, key);
                        assert!(e.depth < 12, "depth {}", e.depth);
                        assert!((e.score - e.depth as f64 / 16.0).abs() < 1e-6, "score {} for depth {}", e.score, e.depth);
                        assert!(e.best != 0 && e.best.trailing_zeros() < 25, "best {:#x}", e.best);
                    }
                }
            });
        }
    });
}

#[test]
fn shared_table_roundtrip() {
    let table = SharedTable::new();
    table.bump_age();
    let entry = TtEntry { key: 0xABCDEF, depth: 7, score: -0.375, bound: Bound::Upper, best: square_bit(3, 4), age: 0 };
    table.put(entry);
    let back = table.get(0xABCDEF).expect("stored entry is readable");
    assert_eq!((back.depth, back.bound, back.best), (7, Bound::Upper, square_bit(3, 4)));
    assert!((back.score - entry.score).abs() < 1e-6);
    assert!(table.get(0xABCDEE).is_none(), "a different key is a miss");
}

#[test]
fn parallel_root_scores_match_single_thread() {
    // Full-window scores are exact, so the parallel version must agree with
    // the single-threaded one on the ranking and (to float precision) the
    // numbers. Repeated because the thread interleaving differs per run.
    let mut seed = 0x9E37_79B9;
    let mut compared = 0;
    for _ in 0..12 {
        let Some(state) = random_position(&mut seed, 10) else { continue };
        let single = root_scores(&state, 4, &[]);
        for threads in [2, 4] {
            let parallel = root_scores_parallel(&state, 4, &[], threads);
            assert_eq!(single.len(), parallel.len());
            // Same best score; every move gets the same score (moves with equal
            // scores may come out in a different order, which is fine).
            assert!((single[0].1 - parallel[0].1).abs() < 1e-4, "best score differs with {threads} threads for {state:?}");
            for (bit, s1) in &single {
                let s2 = parallel.iter().find(|(b, _)| b == bit).map(|(_, s)| *s).expect("every move scored");
                assert!((s1 - s2).abs() < 1e-4, "score {s1} vs {s2} for a move with {threads} threads in {state:?}");
            }
        }
        compared += 1;
    }
    assert!(compared >= 6, "too few positions compared: {compared}");
}

#[test]
fn parallel_best_move_is_legal_deep_and_agrees_at_one_thread() {
    let t = tables();
    let (after_a1, _) = play_move(t, square_bit(0, 0), &State::new());
    let tt = TranspositionTable::new();
    let single = best_move_scored(&after_a1, 6, u64::MAX, &[], &tt, false).0.unwrap();
    for _ in 0..3 {
        let (one, depth1) = best_move_parallel(&after_a1, 6, &[], 1);
        assert_eq!(one.unwrap().0, single.0, "one thread should reproduce the single-threaded move");
        assert_eq!(depth1, 6);
        for threads in [2, 4] {
            let (best, depth) = best_move_parallel(&after_a1, 6, &[], threads);
            let (bit, score) = best.expect("a move");
            assert_eq!(depth, 6, "{threads} threads should complete the target depth");
            assert!(after_a1.empty() & bit != 0, "move must be legal");
            assert!(score.abs() < 1.0, "no forced result exists this early: {score}");
        }
    }
}
