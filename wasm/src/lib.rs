//! WebAssembly entry points. The module owns the whole game state; the page
//! only calls `play` / `ai_play` / `undo` / `redo` and redraws from
//! `board(0)`, `board(1)`.
//!
//! Every export takes and returns plain integers, so no JS glue is needed
//! beyond `WebAssembly.instantiate`.

pub mod game;
pub mod search;

use game::{play_move, square_bit, tables, State, COLS, ROWS};
use search::TranspositionTable;
use std::cell::RefCell;

/// A position occurring this many times ends the game as a draw (as in chess).
pub const REPETITION_LIMIT: usize = 3;

/// One move of the game, with the position it led to.
#[derive(Clone, Copy)]
struct Ply {
    /// Position after the move.
    state: State,
    /// The square played.
    move_bit: u32,
    /// Corner mask of the completed square if the move won.
    won: Option<u32>,
}

/// The game is the list of plies played from the initial position; the
/// winner, the draw and the repetition history are all derived from it.
struct Session {
    plies: Vec<Ply>,
    /// Moves taken back, the most recently undone last.
    redo: Vec<Ply>,
    /// Search results kept for the whole session: the positions explored for
    /// one move are where the next search starts.
    tt: TranspositionTable,
    /// Last `analyse` result, best first: (square index, score for the side
    /// to move, expected continuation as square indices starting with the move).
    analysis: Vec<(usize, f64, Vec<usize>)>,
    analysis_depth: u32,
}

impl Session {
    fn new() -> Self {
        Session { plies: Vec::new(), redo: Vec::new(), tt: TranspositionTable::new(), analysis: Vec::new(), analysis_depth: 0 }
    }
    fn state(&self) -> State {
        self.plies.last().map_or(State::new(), |p| p.state)
    }
    /// The last ply, if it won the game.
    fn winning_ply(&self) -> Option<&Ply> {
        self.plies.last().filter(|p| p.won.is_some())
    }
    fn winner(&self) -> i32 {
        // `play_move` keeps the winner as the side to move.
        self.winning_ply().map_or(-1, |p| p.state.current as i32)
    }
    fn winning_mask(&self) -> u32 {
        self.winning_ply().and_then(|p| p.won).unwrap_or(0)
    }
    /// Keys of every position so far, the initial one first and the current one last.
    fn keys(&self) -> Vec<u64> {
        std::iter::once(State::new().key()).chain(self.plies.iter().map(|p| p.state.key())).collect()
    }
    /// How many times the current position has occurred (1 = first time).
    fn repetitions(&self) -> usize {
        let key = self.state().key();
        self.keys().into_iter().filter(|&k| k == key).count()
    }
    fn draw(&self) -> bool {
        self.winner() < 0 && self.repetitions() >= REPETITION_LIMIT
    }
    fn over(&self) -> bool {
        self.winner() >= 0 || self.draw()
    }
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

/// Start a new game (player 0 to move). Keeps the transposition table: its
/// entries describe positions, which stay valid across games.
#[no_mangle]
pub extern "C" fn reset() {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        s.plies.clear();
        s.redo.clear();
    });
}

/// Bitboard of `player`'s pieces (bit `row * 5 + col`).
#[no_mangle]
pub extern "C" fn board(player: u32) -> u32 {
    SESSION.with(|s| s.borrow().state().boards.get(player as usize).copied().unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn current_player() -> u32 {
    SESSION.with(|s| s.borrow().state().current as u32)
}

/// Winner's player index, or -1 while the game is in progress.
#[no_mangle]
pub extern "C" fn winner() -> i32 {
    SESSION.with(|s| s.borrow().winner())
}

/// Corner mask of the completed square once the game is won, else 0.
#[no_mangle]
pub extern "C" fn winning_mask() -> u32 {
    SESSION.with(|s| s.borrow().winning_mask())
}

/// 1 once the game has ended in a draw by threefold repetition, else 0.
#[no_mangle]
pub extern "C" fn draw() -> u32 {
    SESSION.with(|s| s.borrow().draw() as u32)
}

/// How many times the current position has occurred (1 = first time).
#[no_mangle]
pub extern "C" fn repetitions() -> u32 {
    SESSION.with(|s| s.borrow().repetitions() as u32)
}

/// Play (row, col) for the player to move. Returns 1 if applied, 0 if the
/// square is off-board, occupied, or the game is over. Clears the redo stack.
#[no_mangle]
pub extern "C" fn play(row: u32, col: u32) -> u32 {
    if row as usize >= ROWS || col as usize >= COLS {
        return 0;
    }
    play_bit(square_bit(row as usize, col as usize))
}

/// The square index the AI would play for the side to move, without playing
/// it (a hint), or -1 if the game is over. Same search as `ai_play`.
#[no_mangle]
pub extern "C" fn ai_suggest(max_depth: u32, node_budget: u32) -> i32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        if s.over() {
            return -1;
        }
        let (state, history) = (s.state(), s.keys());
        search::search_depth_reached(&state, max_depth.max(1), node_budget as u64, &history, &mut s.tt)
            .0
            .map_or(-1, |bit| bit.trailing_zeros() as i32)
    })
}

/// Analyse the current position: score every legal move for the side to move
/// under the same kind of budget as `ai_play`. Returns how many moves were
/// scored; read them with `analysis_move` / `analysis_score` (best first) and
/// the depth with `analysis_depth`.
#[no_mangle]
pub extern "C" fn analyse(max_depth: u32, node_budget: u32) -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        s.analysis.clear();
        s.analysis_depth = 0;
        if s.over() {
            return 0;
        }
        let (state, history) = (s.state(), s.keys());
        let (scores, depth) = search::analyse_position(&state, max_depth.max(1), node_budget as u64, &history, &mut s.tt);
        let analysis = scores
            .into_iter()
            .map(|(bit, score)| {
                let line = search::table_line(&s.tt, &state, bit, 8);
                (bit.trailing_zeros() as usize, score, line.into_iter().map(|b| b.trailing_zeros() as usize).collect())
            })
            .collect();
        s.analysis = analysis;
        s.analysis_depth = depth;
        s.analysis.len() as u32
    })
}

#[no_mangle]
pub extern "C" fn analysis_move(i: u32) -> i32 {
    SESSION.with(|s| s.borrow().analysis.get(i as usize).map_or(-1, |(sq, _, _)| *sq as i32))
}

#[no_mangle]
pub extern "C" fn analysis_score(i: u32) -> f64 {
    SESSION.with(|s| s.borrow().analysis.get(i as usize).map_or(0.0, |(_, score, _)| *score))
}

/// The `j`-th square of the `i`-th candidate's expected line (the line starts
/// with the candidate move itself), or -1 past its end.
#[no_mangle]
pub extern "C" fn analysis_line(i: u32, j: u32) -> i32 {
    SESSION.with(|s| {
        s.borrow()
            .analysis
            .get(i as usize)
            .and_then(|(_, _, line)| line.get(j as usize).copied())
            .map_or(-1, |sq| sq as i32)
    })
}

#[no_mangle]
pub extern "C" fn analysis_depth() -> u32 {
    SESSION.with(|s| s.borrow().analysis_depth)
}

/// Let the AI choose and play a move for the player to move. It searches
/// deeper and deeper until `node_budget` positions have been visited or
/// `max_depth` is reached. Returns the square index played, or -1 if none.
#[no_mangle]
pub extern "C" fn ai_play(max_depth: u32, node_budget: u32) -> i32 {
    let chosen = SESSION.with(|s| {
        let mut s = s.borrow_mut();
        if s.over() {
            return None;
        }
        let (state, history) = (s.state(), s.keys());
        search::search_depth_reached(&state, max_depth.max(1), node_budget as u64, &history, &mut s.tt).0
    });
    match chosen {
        Some(bit) if play_bit(bit) == 1 => bit.trailing_zeros() as i32,
        _ => -1,
    }
}

/// Take back the last move. Returns 1 if a move was undone, 0 if there was
/// none. Undoing clears a win or draw.
#[no_mangle]
pub extern "C" fn undo() -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        match s.plies.pop() {
            Some(ply) => {
                s.redo.push(ply);
                1
            }
            None => 0,
        }
    })
}

/// Replay the most recently undone move. Returns 1 if one was replayed, else 0.
#[no_mangle]
pub extern "C" fn redo() -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        match s.redo.pop() {
            Some(ply) => {
                s.plies.push(ply);
                1
            }
            None => 0,
        }
    })
}

/// Number of moves played so far.
#[no_mangle]
pub extern "C" fn move_count() -> u32 {
    SESSION.with(|s| s.borrow().plies.len() as u32)
}

/// Square index (`row * 5 + col`) of the `i`-th move, or -1 if out of range.
#[no_mangle]
pub extern "C" fn move_at(i: u32) -> i32 {
    SESSION.with(|s| s.borrow().plies.get(i as usize).map_or(-1, |p| p.move_bit.trailing_zeros() as i32))
}

/// Bitboard of `player`'s pieces after the first `ply` moves (0 = the empty
/// board). Out-of-range plies give the current position.
#[no_mangle]
pub extern "C" fn board_at(ply: u32, player: u32) -> u32 {
    SESSION.with(|s| {
        let s = s.borrow();
        let state = match ply as usize {
            0 => State::new(),
            n if n <= s.plies.len() => s.plies[n - 1].state,
            _ => s.state(),
        };
        state.boards.get(player as usize).copied().unwrap_or(0)
    })
}

/// Number of undone moves that `redo` can replay.
#[no_mangle]
pub extern "C" fn redo_count() -> u32 {
    SESSION.with(|s| s.borrow().redo.len() as u32)
}

fn play_bit(bit: u32) -> u32 {
    SESSION.with(|s| {
        let mut s = s.borrow_mut();
        let state = s.state();
        if s.over() || state.occupied() & bit != 0 {
            return 0;
        }
        let (next, won) = play_move(tables(), bit, &state);
        s.plies.push(Ply { state: next, move_bit: bit, won });
        s.redo.clear();
        1
    })
}
