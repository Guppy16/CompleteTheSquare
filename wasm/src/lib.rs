//! WebAssembly entry points. The module owns the whole game state; the page
//! only calls `play` / `ai_play` and redraws from `board(0)`, `board(1)`.
//!
//! Every export takes and returns plain integers, so no JS glue is needed
//! beyond `WebAssembly.instantiate`.

pub mod game;
pub mod search;

use game::{play_move, square_bit, tables, State, COLS, ROWS};
use std::cell::RefCell;

struct Session {
    state: State,
    /// Player index of the winner, or -1 while the game is running.
    winner: i32,
    /// Corner mask of the winning square, for highlighting.
    winning_mask: u32,
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session { state: State::new(), winner: -1, winning_mask: 0 });
}

/// Start a new game (player 0 to move).
#[no_mangle]
pub extern "C" fn reset() {
    SESSION.with(|s| *s.borrow_mut() = Session { state: State::new(), winner: -1, winning_mask: 0 });
}

/// Bitboard of `player`'s pieces (bit `row * 5 + col`).
#[no_mangle]
pub extern "C" fn board(player: u32) -> u32 {
    SESSION.with(|s| s.borrow().state.boards.get(player as usize).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn current_player() -> u32 {
    SESSION.with(|s| s.borrow().state.current as u32)
}

/// Winner's player index, or -1 while the game is in progress.
#[no_mangle]
pub extern "C" fn winner() -> i32 {
    SESSION.with(|s| s.borrow().winner)
}

/// Corner mask of the completed square once the game is won, else 0.
#[no_mangle]
pub extern "C" fn winning_mask() -> u32 {
    SESSION.with(|s| s.borrow().winning_mask)
}

/// Play (row, col) for the player to move. Returns 1 if applied, 0 if the
/// square is off-board, occupied, or the game is over.
#[no_mangle]
pub extern "C" fn play(row: u32, col: u32) -> u32 {
    if row as usize >= ROWS || col as usize >= COLS {
        return 0;
    }
    play_bit(square_bit(row as usize, col as usize))
}

/// Let the AI choose and play a move for the player to move, searching
/// `depth` plies. Returns the square index played, or -1 if none.
#[no_mangle]
pub extern "C" fn ai_play(depth: u32) -> i32 {
    let state = SESSION.with(|s| (s.borrow().winner < 0).then(|| s.borrow().state));
    let Some(state) = state else { return -1 };
    match search::best_move(&state, depth.max(1)) {
        Some(bit) if play_bit(bit) == 1 => bit.trailing_zeros() as i32,
        _ => -1,
    }
}

fn play_bit(bit: u32) -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        if s.winner >= 0 || s.state.occupied() & bit != 0 {
            return 0;
        }
        let (next, won) = play_move(tables(), bit, &s.state);
        if let Some(mask) = won {
            s.winner = s.state.current as i32;
            s.winning_mask = mask;
        }
        s.state = next;
        1
    })
}
