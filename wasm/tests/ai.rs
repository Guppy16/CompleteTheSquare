//! Regression tests for the rules engine and the AI.  Run: cargo test --release

use complete_the_square_ai::game::{play_move, square_bit, tables, State};
use complete_the_square_ai::search::{
    best_move_index, negamax, quiescence, Search, DRAW_CONTEMPT, QUIESCENCE_DEPTH, WIN_DEPTH_BONUS, WIN_SCORE,
};

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
    // (Same rules as the real search: a repeated position is a draw worth
    // -DRAW_CONTEMPT to the root player.)
    fn plain(s: &mut Search, state: &State, depth: u32, root: usize, path: &mut Vec<u64>) -> f64 {
        let t = tables();
        let draw = if state.current == root { -DRAW_CONTEMPT } else { DRAW_CONTEMPT };
        if path.contains(&state.key()) {
            return draw;
        }
        if depth == 0 {
            return quiescence(s, state, f64::NEG_INFINITY, f64::INFINITY, QUIESCENCE_DEPTH);
        }
        let empty = state.empty();
        if empty == 0 {
            return draw;
        }
        path.push(state.key());
        let mut best = f64::NEG_INFINITY;
        for i in 0..25 {
            let bit = 1 << i;
            if bit & empty == 0 {
                continue;
            }
            let (child, won) = play_move(t, bit, state);
            if won.is_some() {
                best = WIN_SCORE + WIN_DEPTH_BONUS * depth as f64;
                break;
            }
            best = best.max(-plain(s, &child, depth - 1, root, path));
        }
        path.pop();
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
        let mut search = Search::new(&state, 3, &[], None);
        let expected = plain(&mut search, &state, 3, state.current, &mut Vec::new());
        search.path.clear(); // negamax pushes the root itself, as plain() does
        let actual = negamax(&mut search, &state, 3, f64::NEG_INFINITY, f64::INFINITY);
        assert!((expected - actual).abs() < 1e-12, "{state:?}: {expected} vs {actual}");
    }
}

#[test]
fn ai_behaviour() {
    let best = |s: State| best_move_index(&s, 5, &[]).map(|i| (i / 5, i % 5)).unwrap();
    // Takes an immediate win.
    assert_eq!(best(position(&[(0, 0), (0, 2), (2, 0)], &[(4, 4), (4, 3), (3, 4)], 1)), (3, 3));
    // Blocks an immediate threat.
    assert_eq!(best(position(&[(0, 0), (0, 2), (2, 0)], &[(4, 4)], 1)), (2, 2));
    // Replies to a centre opening with a corner (corners can never be captured).
    let reply = best(position(&[(2, 2)], &[], 1));
    assert!([(0, 0), (0, 4), (4, 0), (4, 4)].contains(&reply), "{reply:?}");
    // Lost to a double threat (green completes a square at B4 or at D4): still
    // block one of them rather than playing the first square in the move order.
    let green = [(0, 0), (0, 3), (2, 0), (2, 1), (3, 0)];
    let red = [(4, 4), (4, 3), (1, 4)];
    let reply = best(position(&green, &red, 1));
    assert!([(3, 1), (3, 3)].contains(&reply), "{reply:?}");
    // Triple threat (B4, C4, D4): every move leaves two, so block one of them
    // rather than capturing elsewhere (E1 would take D1).
    let green = [(0, 0), (0, 3), (1, 0), (1, 2), (2, 0), (2, 1), (2, 3), (3, 0), (4, 2)];
    let red = [(0, 1), (0, 2), (1, 1), (4, 0)];
    let reply = best(position(&green, &red, 1));
    assert!([(3, 1), (3, 2), (3, 3)].contains(&reply), "{reply:?}");
}

#[test]
fn avoids_repetition_and_declares_threefold_draw() {
    // Red threatens the 2x2 at (0,2),(0,3),(1,2),(1,3). Green must block at (1,2),
    // which captures (1,3); red re-playing (1,3) captures (1,2) and restores the
    // threat. Left alone, the two sides repeat forever.
    let green = [(0, 0), (0, 1), (1, 4), (2, 2)];
    let red = [(0, 2), (0, 3), (0, 4), (1, 1), (1, 3)];
    let s1 = position(&green, &red, 0);
    let (s2, _) = play_move(tables(), square_bit(1, 2), &s1); // green blocks and captures

    // With the history in view, red should not walk into the repetition.
    let choice = best_move_index(&s2, 7, &[s1.key(), s2.key()]).map(|i| (i / 5, i % 5)).unwrap();
    assert_ne!(choice, (1, 3), "red repeated the position");

    // The live game ends as a draw when the same position comes up a third time.
    use complete_the_square_ai::{draw, play, reset, winner};
    reset();
    // Reach s2 (red to move) with no captures on the way.
    for &(r, c) in &[(0, 0), (0, 2), (0, 1), (0, 3), (2, 2), (0, 4), (1, 4), (1, 1), (1, 2)] {
        assert_eq!(play(r, c), 1);
    }
    // Red (1,3) captures (1,2) -> s1; green (1,2) captures (1,3) -> s2 again ...
    // s2 has now occurred once; two more cycles make it three.
    for (n, &(r, c)) in [(1, 3), (1, 2), (1, 3), (1, 2)].iter().enumerate() {
        assert_eq!(play(r, c), 1, "move {n} refused");
    }
    assert_eq!(winner(), -1);
    assert_eq!(draw(), 1, "threefold repetition should end the game");
    assert_eq!(play(3, 3), 0, "no moves after the draw");
}

#[test]
fn undo_and_redo() {
    use complete_the_square_ai::{
        board, current_player, draw, move_at, move_count, play, redo, redo_count, reset, undo, winner, winning_mask,
    };
    let snapshot = || {
        let moves: Vec<i32> = (0..move_count()).map(|i| move_at(i)).collect();
        (board(0), board(1), current_player(), winner(), winning_mask(), draw(), moves)
    };

    reset();
    // Green (2,0) on the last move captures red's (2,1),(2,2), flanked by (2,3).
    let moves = [(2, 3), (2, 1), (4, 4), (2, 2), (0, 0), (1, 1), (2, 0)];
    for &(r, c) in &moves {
        assert_eq!(play(r, c), 1);
    }
    assert_eq!(move_count(), 7);
    assert_eq!(move_at(6), 2 * 5);
    assert_eq!(move_at(7), -1);
    assert_eq!(squares(board(1)), vec![(1, 1)]);
    let after_capture = snapshot();

    // Undo past the capture: the captured pieces are back and it is green's move.
    assert_eq!(undo(), 1);
    assert_eq!(move_count(), 6);
    assert_eq!(redo_count(), 1);
    assert_eq!(current_player(), 0);
    assert_eq!(squares(board(1)), vec![(1, 1), (2, 1), (2, 2)]);
    assert_eq!(squares(board(0)), vec![(0, 0), (2, 3), (4, 4)]);
    assert_eq!(undo(), 1);
    assert_eq!(move_count(), 5);
    assert_eq!(redo_count(), 2);

    // Redo restores exactly the position before the undo.
    assert_eq!(redo(), 1);
    assert_eq!(redo(), 1);
    assert_eq!(redo(), 0);
    assert_eq!(snapshot(), after_capture);

    // A new move after an undo discards the redo stack.
    assert_eq!(undo(), 1);
    assert_eq!(play(3, 3), 1);
    assert_eq!(redo_count(), 0);
    assert_eq!(redo(), 0);
    assert_eq!(move_count(), 7);
    assert_eq!(move_at(6), 3 * 5 + 3);

    // Undoing a winning move reopens the game; redoing it wins again.
    // (Green (2,0) recaptures (2,1),(2,2); (0,2) captures (1,1); (2,2) completes the 3x3.)
    for &(r, c) in &[(4, 0), (2, 0), (4, 1), (0, 2), (4, 2), (2, 2)] {
        assert_eq!(play(r, c), 1);
    }
    assert_eq!(winner(), 0, "green completed (0,0),(0,2),(2,0),(2,2)");
    assert_eq!(play(4, 3), 0);
    assert_eq!(undo(), 1);
    assert_eq!((winner(), winning_mask(), current_player()), (-1, 0, 0));
    assert_eq!(redo(), 1);
    assert_eq!(winner(), 0);
    assert_eq!(undo(), 1);
    assert_eq!(play(4, 3), 1, "the game is open again after the undo");

    // Nothing left to undo after reset.
    reset();
    assert_eq!((undo(), redo(), move_count(), redo_count()), (0, 0, 0, 0));
}

#[test]
fn symmetric_positions_share_a_key() {
    use complete_the_square_ai::game::{canonical_key, transform, SYMMETRIES};
    let t = tables();
    let state = position(&[(0, 0), (1, 2), (2, 1)], &[(0, 3), (4, 4)], 1);
    let (key, _) = canonical_key(t, &state);
    for s in 0..SYMMETRIES {
        let twin = State { boards: [transform(t, s, state.boards[0]), transform(t, s, state.boards[1])], current: 1 };
        assert_eq!(canonical_key(t, &twin).0, key, "symmetry {s}");
        // colours swapped, other side to move
        let swapped = State { boards: [twin.boards[1], twin.boards[0]], current: 0 };
        assert_eq!(canonical_key(t, &swapped).0, key, "symmetry {s} + colour swap");
        // and the inverse really undoes the transform
        let back = transform(t, t.sym_inverse[s], twin.boards[0]);
        assert_eq!(back, state.boards[0], "inverse of symmetry {s}");
    }
    // A different position must not collide.
    let other = position(&[(0, 0), (1, 2), (2, 2)], &[(0, 3), (4, 4)], 1);
    assert_ne!(canonical_key(t, &other).0, key);
}
