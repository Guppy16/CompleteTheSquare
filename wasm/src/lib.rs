//! WebAssembly entry points. The module owns the whole game state; the page
//! only calls `play` / `ai_play` and redraws from `board(0)`, `board(1)`.
//!
//! Every export takes and returns plain integers, so no JS glue is needed
//! beyond `WebAssembly.instantiate`.

pub mod game;
pub mod search;

use game::{play_move, square_bit, tables, State, COLS, ROWS};
use std::cell::RefCell;

/// A position occurring this many times ends the game as a draw (as in chess).
pub const REPETITION_LIMIT: usize = 3;

struct Session {
    state: State,
    /// Player index of the winner, or -1 while the game is running.
    winner: i32,
    /// Corner mask of the winning square, for highlighting.
    winning_mask: u32,
    /// True once the game has ended by threefold repetition.
    draw: bool,
    /// Keys of every position so far, the current one last.
    history: Vec<u64>,
}

impl Session {
    fn new() -> Self {
        let state = State::new();
        Session { state, winner: -1, winning_mask: 0, draw: false, history: vec![state.key()] }
    }
    fn over(&self) -> bool {
        self.winner >= 0 || self.draw
    }
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

/// Start a new game (player 0 to move).
#[no_mangle]
pub extern "C" fn reset() {
    SESSION.with(|s| *s.borrow_mut() = Session::new());
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

/// 1 once the game has ended in a draw by threefold repetition, else 0.
#[no_mangle]
pub extern "C" fn draw() -> u32 {
    SESSION.with(|s| s.borrow().draw as u32)
}

/// How many times the current position has occurred (1 = first time).
#[no_mangle]
pub extern "C" fn repetitions() -> u32 {
    SESSION.with(|s| {
        let s = s.borrow();
        let key = s.state.key();
        s.history.iter().filter(|&&k| k == key).count() as u32
    })
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
    let started = SESSION.with(|s| {
        let s = s.borrow();
        (!s.over()).then(|| (s.state, s.history.clone()))
    });
    let Some((state, history)) = started else { return -1 };
    match search::best_move(&state, depth.max(1), &history) {
        Some(bit) if play_bit(bit) == 1 => bit.trailing_zeros() as i32,
        _ => -1,
    }
}

fn play_bit(bit: u32) -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        if s.over() || s.state.occupied() & bit != 0 {
            return 0;
        }
        let (next, won) = play_move(tables(), bit, &s.state);
        if let Some(mask) = won {
            s.winner = s.state.current as i32;
            s.winning_mask = mask;
        }
        s.state = next;
        s.history.push(next.key());
        if won.is_none() && s.history.iter().filter(|&&k| k == next.key()).count() >= REPETITION_LIMIT {
            s.draw = true;
        }
        1
    })
}
