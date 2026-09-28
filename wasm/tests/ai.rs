//! Regression tests for the rules engine and the AI.  Run: cargo test --release

use complete_the_square_ai::game::{play_move, square_bit, tables, State};
use complete_the_square_ai::search::{best_move_index, evaluator, negamax, WIN_DEPTH_BONUS, WIN_SCORE};

fn position(p0: &[(usize, usize)], p1: &[(usize, usize)], current: usize) -> State {
    let bits = |sq: &[(usize, usize)]| sq.iter().fold(0, |m, &(r, c)| m | square_bit(r, c));
    State { boards: [bits(p0), bits(p1)], current }
}

fn squares(board: u32) -> Vec<(usize, usize)> {
    (0..25).filter(|i| board & (1 << i) != 0).map(|i| (i / 5, i % 5)).collect()
}

#[test]
fn captures() {
    // Player 0 plays (2,0); player 1's run (2,1),(2,2) is flanked by (2,3) -> captured.
    // The diagonal (1,1) is not flanked (nothing at (0,2)) -> stays.
    let state = position(&[(2, 3)], &[(2, 1), (2, 2), (1, 1)], 0);
    let (next, won) = play_move(tables(), square_bit(2, 0), &state);
    assert!(won.is_none());
    assert_eq!(squares(next.boards[1]), vec![(1, 1)]);
    assert_eq!(squares(next.boards[0]), vec![(2, 0), (2, 3)]);
}

#[test]
fn alpha_beta_matches_plain_minimax() {
    // Pruning and move ordering must never change the value of a position.
    fn plain(state: &State, depth: u32) -> f64 {
        let (t, ev) = (tables(), evaluator());
        if depth == 0 {
            return ev.evaluate(t, state);
        }
        let empty = state.empty();
        if empty == 0 {
            return 0.0;
        }
        let mut best = f64::NEG_INFINITY;
        for i in 0..25 {
            let bit = 1 << i;
            if bit & empty == 0 {
                continue;
            }
            let (child, won) = play_move(t, bit, state);
            if won.is_some() {
                return WIN_SCORE + WIN_DEPTH_BONUS * depth as f64;
            }
            best = best.max(-plain(&child, depth - 1));
        }
        best
    }

    // Tiny deterministic RNG (xorshift) so the test needs no dependencies.
    let mut seed: u32 = 0x9E37_79B9;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };

    let t = tables();
    for _ in 0..40 {
        let mut state = State::new();
        let mut game_over = false;
        for _ in 0..(next() % 12) {
            let empty = state.empty();
            let legal: Vec<u32> = (0..25).map(|i| 1 << i).filter(|b| b & empty != 0).collect();
            let (child, won) = play_move(t, legal[next() as usize % legal.len()], &state);
            state = child;
            if won.is_some() {
                game_over = true;
                break;
            }
        }
        if game_over {
            continue;
        }
        let expected = plain(&state, 3);
        let mut killers = [0; 4];
        let actual = negamax(t, evaluator(), &state, 3, f64::NEG_INFINITY, f64::INFINITY, &mut killers);
        assert!((expected - actual).abs() < 1e-12, "{state:?}: {expected} vs {actual}");
    }
}

#[test]
fn ai_behaviour() {
    let best = |s: State| best_move_index(&s, 5).map(|i| (i / 5, i % 5)).unwrap();
    // Takes an immediate win.
    assert_eq!(best(position(&[(0, 0), (0, 2), (2, 0)], &[(4, 4), (4, 3), (3, 4)], 1)), (3, 3));
    // Blocks an immediate threat.
    assert_eq!(best(position(&[(0, 0), (0, 2), (2, 0)], &[(4, 4)], 1)), (2, 2));
    // Replies to a centre opening with a corner (corners can never be captured).
    let reply = best(position(&[(2, 2)], &[], 1));
    assert!([(0, 0), (0, 4), (4, 0), (4, 4)].contains(&reply), "{reply:?}");
}
