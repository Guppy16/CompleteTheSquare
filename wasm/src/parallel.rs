//! Multi-threaded search for the native tools (`examples/analyse`, arena).
//! Not compiled for WebAssembly, which has no threads.
//!
//! Two strategies, both reusing the single-threaded search unchanged:
//!
//! - **Lazy SMP** (`best_move_parallel`): every thread runs the same
//!   iterative deepening on the same position, sharing one table. Threads
//!   start at different depths and try moves in different orders, so they
//!   explore different parts of the tree and hand each other results through
//!   the table. The first thread to finish the target depth stops the rest.
//! - **Root splitting** (`root_scores_parallel`): scoring every root move
//!   with a full window is embarrassingly parallel: deal the moves out to
//!   the threads, all sharing the table.
//!
//! The shared table (`SharedTable`) is lock-free: each slot is two atomic
//! 64-bit words, the packed entry and the key XOR-ed with it, so a torn
//! write (two threads storing at once) reads back as a miss, never as a
//! wrong entry.

use crate::game::{play_move, tables, Bit, State};
use crate::search::{
    evaluator, ordered_moves, search_root, should_replace, table_index, table_line, Bound, Candidate, Evaluator, Search,
    Table, TtEntry, WIN_DEPTH_BONUS, WIN_SCORE,
};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

const TT_SIZE: usize = 1 << 18;

/// A transposition table that several threads can read and write at once.
pub struct SharedTable {
    /// Per slot: [key ^ data, data]. See `pack` / `unpack`.
    slots: Vec<[AtomicU64; 2]>,
    age: AtomicU32,
}

impl Default for SharedTable {
    fn default() -> Self {
        Self::new()
    }
}

// Entry layout inside the 64-bit data word:
//   bits  0..32  score as f32 bits
//   bits 32..40  depth (u8)
//   bits 40..42  bound (0 exact, 1 lower, 2 upper)
//   bits 42..48  best move as square index + 1 (0 = none)
//   bits 48..63  age (15 bits)
//   bit  63      path-dependent flag
fn pack(e: &TtEntry) -> u64 {
    let score = (e.score as f32).to_bits() as u64;
    let depth = (e.depth.min(255) as u64) << 32;
    let bound = (match e.bound {
        Bound::Exact => 0u64,
        Bound::Lower => 1,
        Bound::Upper => 2,
    }) << 40;
    let best = if e.best == 0 { 0 } else { (e.best.trailing_zeros() as u64 + 1) << 42 };
    let age = ((e.age & 0x7FFF) as u64) << 48;
    let flag = (e.path_dependent as u64) << 63;
    score | depth | bound | best | age | flag
}

fn unpack(key: u64, data: u64) -> TtEntry {
    let best_index = (data >> 42) & 0x3F;
    TtEntry {
        key,
        depth: ((data >> 32) & 0xFF) as u32,
        score: f32::from_bits(data as u32) as f64,
        bound: match (data >> 40) & 0x3 {
            0 => Bound::Exact,
            1 => Bound::Lower,
            _ => Bound::Upper,
        },
        best: if best_index == 0 { 0 } else { 1 << (best_index - 1) },
        age: ((data >> 48) & 0x7FFF) as u32,
        path_dependent: (data >> 63) & 1 == 1,
    }
}

impl SharedTable {
    pub fn new() -> Self {
        let slots = (0..TT_SIZE).map(|_| [AtomicU64::new(0), AtomicU64::new(0)]).collect();
        SharedTable { slots, age: AtomicU32::new(0) }
    }
}

impl Table for SharedTable {
    fn get(&self, key: u64) -> Option<TtEntry> {
        let slot = &self.slots[table_index(key)];
        let w0 = slot[0].load(Ordering::Relaxed);
        let w1 = slot[1].load(Ordering::Relaxed);
        // A torn write (w0 from one entry, w1 from another) fails this check.
        (w0 ^ w1 == key && w1 != 0).then(|| unpack(key, w1))
    }

    fn put(&self, mut entry: TtEntry) {
        entry.age = self.age() & 0x7FFF;
        let slot = &self.slots[table_index(entry.key)];
        let w0 = slot[0].load(Ordering::Relaxed);
        let w1 = slot[1].load(Ordering::Relaxed);
        let existing = unpack(w0 ^ w1, w1); // whatever position is there, torn or not
        if w1 == 0 || should_replace(&existing, &entry, entry.age) {
            let data = pack(&entry);
            slot[1].store(data, Ordering::Relaxed);
            slot[0].store(entry.key ^ data, Ordering::Relaxed);
        }
    }

    fn age(&self) -> u32 {
        self.age.load(Ordering::Relaxed)
    }

    fn bump_age(&self) {
        self.age.fetch_add(1, Ordering::Relaxed);
    }
}

/// An evaluator whose static move order is nudged for thread `i`: the first
/// few entries are rotated so threads try different first moves, but the
/// order stays corner-first overall (rotating the whole list made the high
/// threads search in a bad order and prune badly, which slowed everything).
fn rotated_evaluator(i: usize) -> Evaluator {
    let mut ev = evaluator().clone();
    let window = 1 + i % 4; // thread 0: unchanged; others rotate the first 2..5 moves
    ev.move_order[..window].rotate_left(if window > 1 { 1 } else { 0 });
    ev
}

/// Lazy SMP: the best move for the side to move, searching `max_depth` plies
/// with `threads` threads. Returns the deepest completed result and its depth.
pub fn best_move_parallel(state: &State, max_depth: u32, history: &[u64], threads: usize) -> (Option<(Bit, f64)>, u32) {
    let threads = threads.max(1);
    let tt = SharedTable::new();
    tt.bump_age();
    let stop = AtomicBool::new(false);
    let evaluators: Vec<Evaluator> = (0..threads).map(rotated_evaluator).collect();
    // (depth, result) per completed iteration, from any thread.
    let results: Mutex<Vec<(u32, usize, Option<(Bit, f64)>)>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for (i, ev) in evaluators.iter().enumerate() {
            let (tt, stop, results) = (&tt, &stop, &results);
            scope.spawn(move || {
                let mut s = Search::new(state, max_depth, history, Some(tt)).with_evaluator(ev).with_stop(stop);
                let mut best = None;
                // Odd threads start one ply deeper so the threads diverge at once.
                for depth in (1 + (i % 2) as u32)..=max_depth.max(1) {
                    let first = best.map_or(0, |(b, _)| b);
                    let result = search_root(&mut s, state, depth, first, i);
                    if s.aborted {
                        break;
                    }
                    best = result;
                    results.lock().unwrap().push((depth, i, best));
                    let forced = best.map_or(true, |(_, sc)| sc.abs() >= WIN_SCORE);
                    if depth >= max_depth || forced {
                        stop.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            });
        }
    });

    let results = results.into_inner().unwrap();
    // Deepest completed iteration; among equals the lowest thread index.
    results
        .iter()
        .max_by_key(|(depth, i, _)| (*depth, usize::MAX - *i))
        .map_or((None, 0), |(depth, _, best)| (*best, *depth))
}

/// Root splitting: every legal move scored with a full window at `depth`,
/// best first. The moves form a work queue: each thread takes the next
/// unscored move when it finishes one, so no thread idles while the slowest
/// moves are still being searched (dealing them out in advance left most
/// cores idle for the last third of a deep run).
pub fn root_scores_parallel(state: &State, depth: u32, history: &[u64], threads: usize) -> Vec<(Bit, f64)> {
    root_analysis_parallel(state, depth, history, threads).into_iter().map(|c| (c.bit, c.score)).collect()
}

/// As `root_scores_parallel`, and for each move the expected continuation
/// read from the shared table after the scoring (the same way the page's
/// analysis panel gets its lines). Free, where re-searching for a line
/// costs as much again as the scoring did.
pub fn root_analysis_parallel(state: &State, depth: u32, history: &[u64], threads: usize) -> Vec<Candidate> {
    let threads = threads.max(1);
    let t = tables();
    let tt = SharedTable::new();
    tt.bump_age();
    let moves = ordered_moves(evaluator(), state.empty(), 0, 0);
    let next_move = AtomicUsize::new(0);
    let evaluators: Vec<Evaluator> = (0..threads).map(rotated_evaluator).collect();
    let out: Mutex<Vec<(Bit, f64)>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for ev in evaluators.iter() {
            let (tt, out, moves, next_move) = (&tt, &out, &moves, &next_move);
            scope.spawn(move || {
                let mut s = Search::new(state, depth, history, Some(tt)).with_evaluator(ev);
                loop {
                    let i = next_move.fetch_add(1, Ordering::Relaxed);
                    let Some(bit) = moves.get(i) else { break };
                    let (child, won) = play_move(t, *bit, state);
                    let score = if won.is_some() {
                        WIN_SCORE + WIN_DEPTH_BONUS * depth as f64
                    } else {
                        -crate::search::negamax(&mut s, &child, depth - 1, f64::NEG_INFINITY, f64::INFINITY)
                    };
                    out.lock().unwrap().push((*bit, score));
                }
            });
        }
    });

    let mut out = out.into_inner().unwrap();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    out.into_iter().map(|(bit, score)| Candidate { bit, score, line: table_line(&tt, state, bit, 16) }).collect()
}

/// The expected line, like `search::principal_variation`, using the parallel search.
pub fn principal_variation_parallel(state: &State, depth: u32, history: &[u64], threads: usize) -> Vec<Bit> {
    let t = tables();
    let mut line = Vec::new();
    let mut state = *state;
    let mut history = history.to_vec();
    for remaining in (1..=depth).rev() {
        let Some((bit, _)) = best_move_parallel(&state, remaining, &history, threads).0 else { break };
        line.push(bit);
        let (child, won) = play_move(t, bit, &state);
        if won.is_some() {
            break;
        }
        state = child;
        history.push(state.key());
    }
    line
}
