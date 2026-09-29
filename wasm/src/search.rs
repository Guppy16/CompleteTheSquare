//! Evaluation and negamax alpha-beta search. A direct port of `minimax.py`.

use crate::game::{bit_index, canonical_key, index_to_square, play_move, tables, Bit, State, Tables, COLS, N, ROWS};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub const WIN_SCORE: f64 = 1.0;
pub const WIN_DEPTH_BONUS: f64 = 0.01;
pub const W_MATERIAL: f64 = 0.25;
pub const W_POSITION: f64 = 0.10;
pub const W_THREATS: f64 = 0.15;
pub const MAX_THREATS: i32 = 3;
/// A repeated position counts as a draw. From the AI's side a draw scores
/// -DRAW_CONTEMPT (and +DRAW_CONTEMPT for the opponent), so the AI only
/// repeats when every alternative looks worse than this.
pub const DRAW_CONTEMPT: f64 = 0.2;
/// At a leaf, keep searching capture moves this many plies further so the
/// evaluation is never taken in the middle of a capture exchange.
pub const QUIESCENCE_DEPTH: u32 = 4;
/// Transposition table size (entries); a power of two.
const TT_BITS: u32 = 18;
const TT_SIZE: usize = 1 << TT_BITS; // 262k entries of 32 bytes: 8 MB

#[derive(Clone)]
pub struct Evaluator {
    corner_mask: u32,
    edge_mask: u32,
    position_norm: f64,
    material_norm: f64,
    /// Corner-first move ordering (also the tie-break between equal moves).
    pub move_order: Vec<Bit>,
}

pub fn evaluator() -> &'static Evaluator {
    static EVALUATOR: OnceLock<Evaluator> = OnceLock::new();
    EVALUATOR.get_or_init(Evaluator::new)
}

impl Evaluator {
    fn new() -> Self {
        let (mut corner_mask, mut edge_mask) = (0u32, 0u32);
        for i in 0..N {
            let (r, c) = index_to_square(i);
            let on_row_edge = r == 0 || r == ROWS - 1;
            let on_col_edge = c == 0 || c == COLS - 1;
            if on_row_edge && on_col_edge {
                corner_mask |= 1 << i;
            } else if on_row_edge || on_col_edge {
                edge_mask |= 1 << i;
            }
        }
        let corner_distance = |i: usize| {
            let (r, c) = index_to_square(i);
            r.min(ROWS - 1 - r) + c.min(COLS - 1 - c)
        };
        let mut order: Vec<usize> = (0..N).collect();
        order.sort_by_key(|&i| corner_distance(i)); // stable, like Python's sorted
        Evaluator {
            corner_mask,
            edge_mask,
            position_norm: (2 * corner_mask.count_ones() + edge_mask.count_ones()) as f64,
            material_norm: N as f64,
            move_order: order.into_iter().map(|i| 1 << i).collect(),
        }
    }

    /// Does `mine` have a square with 3 corners and the 4th empty (a win next move)?
    pub fn has_threat(&self, t: &Tables, mine: u32, theirs: u32) -> bool {
        t.all_corner_masks.iter().any(|&mask| theirs & mask == 0 && (mine & mask).count_ones() == 3)
    }

    /// How many squares `mine` can complete next move.
    pub fn count_threats(&self, t: &Tables, mine: u32, theirs: u32) -> usize {
        t.all_corner_masks.iter().filter(|&&mask| theirs & mask == 0 && (mine & mask).count_ones() == 3).count()
    }

    /// Bitmask of the empty squares where `mine` would complete a square.
    pub fn winning_squares(&self, t: &Tables, mine: u32, theirs: u32) -> u32 {
        t.all_corner_masks
            .iter()
            .filter(|&&mask| theirs & mask == 0 && (mine & mask).count_ones() == 3)
            .fold(0, |acc, &mask| acc | (mask & !mine))
    }

    fn threat_difference(&self, t: &Tables, mine: u32, theirs: u32) -> i32 {
        let mut diff = 0;
        for &mask in &t.all_corner_masks {
            let (m, th) = (mine & mask, theirs & mask);
            if th == 0 {
                if m.count_ones() == 3 {
                    diff += 1;
                }
            } else if m == 0 && th.count_ones() == 3 {
                diff -= 1;
            }
        }
        diff
    }

    /// Heuristic score of `state` for the player about to move.
    pub fn evaluate(&self, t: &Tables, state: &State) -> f64 {
        let mine = state.boards[state.current];
        let theirs = state.boards[1 - state.current];

        let material = (mine.count_ones() as i32 - theirs.count_ones() as i32) as f64 / self.material_norm;
        let positional = |b: u32| (2 * (b & self.corner_mask).count_ones() + (b & self.edge_mask).count_ones()) as i32;
        let position = (positional(mine) - positional(theirs)) as f64 / self.position_norm;
        let threat_diff = self.threat_difference(t, mine, theirs).clamp(-MAX_THREATS, MAX_THREATS);
        let threats = threat_diff as f64 / MAX_THREATS as f64;

        W_MATERIAL * material + W_POSITION * position + W_THREATS * threats
    }
}

/// Legal move bits: the table's best move first, then the killer, then corner-out order.
pub(crate) fn ordered_moves(ev: &Evaluator, empty: u32, first: Bit, killer: Bit) -> Vec<Bit> {
    let mut out = Vec::with_capacity(empty.count_ones() as usize);
    if first & empty != 0 {
        out.push(first);
    }
    if killer & empty != 0 && killer != first {
        out.push(killer);
    }
    out.extend(ev.move_order.iter().copied().filter(|&b| b & empty != 0 && b != first && b != killer));
    out
}

/// What a transposition-table score means, given the (alpha, beta) window it
/// was found with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    Exact,
    Lower, // the true score is at least this (search was cut off)
    Upper, // the true score is at most this (no move reached alpha)
}

#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub key: u64,
    pub depth: u32,
    pub score: f64,
    pub bound: Bound,
    pub best: Bit,
    /// Which search stored it (see `Table::age`).
    pub age: u32,
}

/// A store of positions already searched, keyed by the canonical `State::key`.
/// Two implementations: `TranspositionTable` for one thread (used by the
/// WebAssembly build) and `parallel::SharedTable` for several.
///
/// `put` takes `&self` so a table can be shared; the replacement policy is
/// the implementation's, but both use the same one: on a collision the slot
/// is replaced if it is from an older search or searched less deep than the
/// newcomer, so within one search the deep entries near the root (the ones
/// worth keeping, and the ones the expected lines are read from) survive the
/// churn of millions of shallow leaves.
pub trait Table {
    fn get(&self, key: u64) -> Option<TtEntry>;
    fn put(&self, entry: TtEntry);
    /// Which search we are in; entries carry the age they were stored at.
    fn age(&self) -> u32;
    /// Call once at the start of every search: entries from earlier searches
    /// become replaceable.
    fn bump_age(&self);
}

/// Should `newcomer` replace `slot` (which may hold a different position)?
pub fn should_replace(slot: &TtEntry, newcomer: &TtEntry, age: u32) -> bool {
    slot.key == newcomer.key || slot.age != age || newcomer.depth >= slot.depth
}

/// Slot index of a key: the top TT_BITS bits of a multiplicative hash.
pub fn table_index(key: u64) -> usize {
    (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - TT_BITS)) as usize
}

/// The single-threaded table: a fixed array of entries, 8 MB. Lives for the
/// whole game: the positions explored while choosing one move are exactly
/// the ones the next search starts from, so entries carry over.
pub struct TranspositionTable {
    entries: Vec<Cell<TtEntry>>,
    age: Cell<u32>,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TranspositionTable {
    pub fn new() -> Self {
        let empty = TtEntry { key: 0, depth: 0, score: 0.0, bound: Bound::Exact, best: 0, age: 0 };
        TranspositionTable { entries: vec![Cell::new(empty); TT_SIZE], age: Cell::new(0) }
    }
}

impl Table for TranspositionTable {
    fn get(&self, key: u64) -> Option<TtEntry> {
        let e = self.entries[table_index(key)].get();
        (e.key == key).then_some(e)
    }

    fn put(&self, mut entry: TtEntry) {
        entry.age = self.age.get();
        let slot = &self.entries[table_index(entry.key)];
        if should_replace(&slot.get(), &entry, entry.age) {
            slot.set(entry);
        }
    }

    fn age(&self) -> u32 {
        self.age.get()
    }

    fn bump_age(&self) {
        self.age.set(self.age.get().wrapping_add(1));
    }
}

/// Everything one search needs besides the position.
pub struct Search<'a> {
    pub tables: &'static Tables,
    pub evaluator: &'a Evaluator,
    /// The player the search is for (draw scores are relative to them).
    pub root_player: usize,
    /// Keys of every position so far: the game's history, then the current line.
    pub path: Vec<u64>,
    /// killers[depth] = the move that last caused a cut-off at that depth.
    pub killers: Vec<Bit>,
    /// The table shared across the game's searches; `None` disables it.
    tt: Option<&'a dyn Table>,
    /// Nodes visited so far (for the node budget).
    pub nodes: u64,
    /// Stop once this many nodes have been visited (u64::MAX = never).
    pub node_budget: u64,
    /// Another thread can ask this search to stop (parallel search).
    stop: Option<&'a AtomicBool>,
    /// Set when the budget ran out mid-search; results after that are garbage.
    pub aborted: bool,
    /// How many repetition draws have been returned so far. A node whose
    /// subtree contained one has a path-dependent value and is not stored in
    /// the table (see the note in `negamax`).
    repetition_hits: u64,
    /// Table stores skipped because the subtree hit a repetition (statistics).
    pub stores_skipped: u64,
}

impl<'a> Search<'a> {
    /// A search using `tt` (pass `None` for plain alpha-beta). The caller
    /// bumps the table's age once per search (see `Table::bump_age`).
    pub fn new(root: &State, depth: u32, history: &[u64], tt: Option<&'a dyn Table>) -> Self {
        let mut path = history.to_vec();
        if path.last() != Some(&root.key()) {
            path.push(root.key());
        }
        Search {
            tables: tables(),
            evaluator: evaluator(),
            root_player: root.current,
            path,
            killers: vec![0; depth as usize + 1],
            tt,
            nodes: 0,
            node_budget: u64::MAX,
            stop: None,
            aborted: false,
            repetition_hits: 0,
            stores_skipped: 0,
        }
    }

    /// Use a different evaluator (the parallel search gives each thread its
    /// own move order so they explore differently).
    pub fn with_evaluator(mut self, evaluator: &'a Evaluator) -> Self {
        self.evaluator = evaluator;
        self
    }

    /// Abort as soon as `flag` becomes true.
    pub fn with_stop(mut self, flag: &'a AtomicBool) -> Self {
        self.stop = Some(flag);
        self
    }

    fn tt_get(&self, key: u64) -> Option<TtEntry> {
        self.tt.and_then(|t| t.get(key))
    }

    fn tt_put(&mut self, entry: TtEntry) {
        if let Some(t) = self.tt {
            t.put(entry);
        }
    }

    fn out_of_time(&self) -> bool {
        self.nodes >= self.node_budget || self.stop.is_some_and(|f| f.load(Ordering::Relaxed))
    }

    /// Score of a draw for the player to move in `state`.
    ///
    /// Caveat: this is relative to the root player, but table entries are
    /// keyed by position only (colour-swapped twins included) and persist
    /// across searches for either side, so an entry on a repetition line can
    /// carry the other side's sign of the contempt: an error of at most
    /// 2 * DRAW_CONTEMPT, only on lines that repeat. Accepted for simplicity.
    fn draw_score(&self, state: &State) -> f64 {
        if state.current == self.root_player { -DRAW_CONTEMPT } else { DRAW_CONTEMPT }
    }
}

/// Bits of the empty squares where the side to move would capture something.
fn capture_moves(t: &Tables, state: &State) -> Vec<Bit> {
    let own = state.boards[state.current];
    let opp = state.boards[1 - state.current];
    let mut out = Vec::new();
    let mut empty = state.empty();
    while empty != 0 {
        let bit = empty & empty.wrapping_neg();
        empty ^= bit;
        let captures = t.capture_rays[bit_index(bit)].iter().any(|ray| {
            let mut seen_opp = false;
            for &b in ray {
                if opp & b != 0 {
                    seen_opp = true;
                } else {
                    return seen_opp && own & b != 0;
                }
            }
            false
        });
        if captures {
            out.push(bit);
        }
    }
    out
}

/// Quiescence search: evaluate only positions where no capture is pending.
/// The side to move may "stand pat" (take the static evaluation) or play a
/// capture, whichever is better for them.
pub fn quiescence(s: &mut Search, state: &State, mut alpha: f64, beta: f64, qdepth: u32) -> f64 {
    s.nodes += 1;
    let (t, ev) = (s.tables, s.evaluator);
    let (mine, theirs) = (state.boards[state.current], state.boards[1 - state.current]);
    if ev.has_threat(t, mine, theirs) {
        return WIN_SCORE; // we complete a square next move
    }
    let stand_pat = ev.evaluate(t, state);
    if qdepth == 0 || stand_pat >= beta {
        return stand_pat;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }
    let mut best = stand_pat;
    for bit in capture_moves(t, state) {
        let (child, won) = play_move(t, bit, state);
        if won.is_some() {
            return WIN_SCORE;
        }
        let score = -quiescence(s, &child, -beta, -alpha, qdepth - 1);
        if score > best {
            best = score;
        }
        if best > alpha {
            alpha = best;
        }
        if alpha >= beta {
            break;
        }
    }
    best
}

pub fn negamax(s: &mut Search, state: &State, depth: u32, mut alpha: f64, mut beta: f64) -> f64 {
    s.nodes += 1;
    if s.out_of_time() {
        s.aborted = true; // out of budget (or told to stop): unwind; the caller discards this iteration
    }
    if s.aborted {
        return 0.0;
    }
    let key = state.key();
    // A position we have already been through can only lead to a draw by
    // repetition, whatever the evaluation says about it.
    if s.path.contains(&key) {
        s.repetition_hits += 1;
        return s.draw_score(state);
    }
    if depth == 0 {
        return quiescence(s, state, alpha, beta, QUIESCENCE_DEPTH);
    }
    let empty = state.empty();
    if empty == 0 {
        return s.draw_score(state);
    }

    // Have we searched this position (or one of its 16 symmetric twins)
    // before, at least this deep? Table entries live in the canonical frame,
    // so the stored best move is mapped back through the inverse symmetry.
    let (tt_key, sym) = canonical_key(s.tables, state);
    let mut first = 0;
    if let Some(e) = s.tt_get(tt_key) {
        if e.best != 0 {
            first = 1 << s.tables.sym_square[s.tables.sym_inverse[sym]][bit_index(e.best)];
        }
        if e.depth >= depth {
            match e.bound {
                Bound::Exact => return e.score,
                Bound::Lower => alpha = alpha.max(e.score),
                Bound::Upper => beta = beta.min(e.score),
            }
            if alpha >= beta {
                return e.score;
            }
        }
    }
    let alpha_in = alpha;
    let repetitions_before = s.repetition_hits;

    s.path.push(key);
    let mut best = f64::NEG_INFINITY;
    let mut best_bit = 0;
    for bit in ordered_moves(s.evaluator, empty, first, s.killers[depth as usize]) {
        let (child, won) = play_move(s.tables, bit, state);
        if won.is_some() {
            best = WIN_SCORE + WIN_DEPTH_BONUS * depth as f64;
            best_bit = bit;
            break;
        }
        let score = -negamax(s, &child, depth - 1, -beta, -alpha);
        if score > best {
            best = score;
            best_bit = bit;
        }
        if best > alpha {
            alpha = best;
        }
        if alpha >= beta {
            s.killers[depth as usize] = bit;
            break;
        }
    }
    s.path.pop();

    let bound = if best >= beta { Bound::Lower } else if best <= alpha_in { Bound::Upper } else { Bound::Exact };
    // A value that depends on a repetition somewhere below is only valid on
    // this line: the same position reached another way is not a repetition.
    // Storing it would serve a "draw" where there is none, hiding a forced
    // result, so such nodes are not stored (chess engines' GHI problem).
    if !s.aborted && s.repetition_hits != repetitions_before {
        s.stores_skipped += 1;
    }
    if !s.aborted && s.repetition_hits == repetitions_before {
        let best_canonical = if best_bit == 0 { 0 } else { 1 << s.tables.sym_square[sym][bit_index(best_bit)] };
        s.tt_put(TtEntry { key: tt_key, depth, score: best, bound, best: best_canonical, age: 0 });
    }
    best
}

/// One full-width search of the root to `depth`, trying `first` first.
/// Returns (best move, its score).
/// `rotate` shifts the root move order (parallel threads each start from a
/// different root move so they split the work); 0 leaves it unchanged.
pub(crate) fn search_root(s: &mut Search, state: &State, depth: u32, first: Bit, rotate: usize) -> Option<(Bit, f64)> {
    let empty = state.empty();
    let mut best: Option<(Bit, f64)> = None;
    let (mut alpha, beta) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut moves = ordered_moves(s.evaluator, empty, first, 0);
    // Keep the previous iteration's best move in front; rotate the rest.
    let keep = usize::from(first & empty != 0);
    if moves.len() > keep + 1 {
        let tail = &mut moves[keep..];
        let n = tail.len();
        tail.rotate_left(rotate % n);
    }
    for bit in moves {
        let (child, won) = play_move(s.tables, bit, state);
        if won.is_some() {
            return Some((bit, WIN_SCORE + WIN_DEPTH_BONUS * depth as f64));
        }
        let score = -negamax(s, &child, depth - 1, -beta, -alpha);
        if best.map_or(true, |(_, b)| score > b) {
            best = Some((bit, score));
        }
        alpha = alpha.max(score);
    }
    match best {
        Some((_, score)) if score <= -WIN_SCORE => Some(best_losing_move(s, state, depth)),
        other => other,
    }
}

/// Every move loses. Alpha-beta only gives bounds for the non-best moves, so
/// re-score them all with a full window (cheap: the table holds most of it)
/// and pick, in order: the move that loses latest; the one that leaves the
/// opponent the fewest immediate wins; one that sits on an opponent's winning
/// square (a visible block, which is what a human would do). So a human
/// still has to find the remaining win.
fn best_losing_move(s: &mut Search, state: &State, depth: u32) -> (Bit, f64) {
    let (theirs, mine) = (state.boards[1 - state.current], state.boards[state.current]);
    let their_wins = s.evaluator.winning_squares(s.tables, theirs, mine);
    // (score, -threats left, blocks): larger is better
    let mut best: Option<(Bit, (f64, i32, bool))> = None;
    for bit in ordered_moves(s.evaluator, state.empty(), 0, 0) {
        let (child, _) = play_move(s.tables, bit, state);
        let score = -negamax(s, &child, depth - 1, f64::NEG_INFINITY, f64::INFINITY);
        let (c_mine, c_theirs) = (child.boards[child.current], child.boards[1 - child.current]);
        let threats = s.evaluator.count_threats(s.tables, c_mine, c_theirs) as i32;
        let rank = (score, -threats, bit & their_wins != 0);
        if best.map_or(true, |(_, b)| rank > b) {
            best = Some((bit, rank));
        }
    }
    let (bit, (score, _, _)) = best.expect("a lost position still has legal moves");
    (bit, score)
}

/// Opening book: the reply to each distinct first move, found by searching to
/// depth 14 offline (`examples/analyse.rs`), far deeper than the page can
/// afford. Squares are indices (row * 5 + col) in the identity frame; the
/// other 19 first moves are rotations/reflections of these six and are mapped
/// through the symmetry tables. A `None` reply means "not computed yet".
const OPENING_BOOK: [(usize, Option<usize>); 6] = [
    (0, Some(6)),   // A1 -> B2  (-0.27; C3, the search's own choice, loses in 13 plies)
    (1, Some(0)),   // B1 -> A1  (-0.20; B2/D4 -0.27; E1, A5, D1, A2, A4 lose)
    (2, Some(0)),   // C1 -> A1  (-0.09, tied with E1)
    (6, Some(0)),   // B2 -> A1  (-0.09)
    (7, Some(0)),   // C2 -> A1  (0.00, tied with E1; A4, E4, B5, D5 lose; A1/E1/B1 all 40/40 in play)
    (12, Some(0)),  // C3 -> A1  (see below)
];
// Every entry is also validated by play (`examples/arena`), not just by its
// depth-14 score: after 1. C3 the search rated the edge midpoint C5 (-0.13)
// a hair above the corner A1 (-0.16), but in 40 arena games red won 13 with
// C5 and 40 with A1. A 0.03 gap at depth 14 is evaluation noise.

/// The book's reply when `state` is one piece into the game, else None.
pub fn book_move(state: &State) -> Option<Bit> {
    if state.occupied().count_ones() != 1 {
        return None;
    }
    let t = tables();
    let square = bit_index(state.occupied());
    for s in 0..crate::game::SYMMETRIES {
        let canonical = t.sym_square[s][square];
        if let Some(&(_, Some(reply))) = OPENING_BOOK.iter().find(|(first, _)| *first == canonical) {
            return Some(1 << t.sym_square[t.sym_inverse[s]][reply]);
        }
    }
    None
}

/// The line the engine expects after searching `state` to `depth`: the best
/// move, then the best reply searched one ply shallower, and so on. (Walking
/// stored best moves through the table is cheaper but the table is small and
/// early entries get overwritten, so the line would be cut short.)
pub fn principal_variation(state: &State, depth: u32, history: &[u64]) -> Vec<Bit> {
    let t = tables();
    let tt = TranspositionTable::new();
    let mut line = Vec::new();
    let mut state = *state;
    let mut history = history.to_vec();
    for remaining in (1..=depth).rev() {
        let Some(bit) = search_depth_reached(&state, remaining, u64::MAX, &history, &tt).0 else { break };
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

/// Analysis: every legal move scored for the side to move, best first, and
/// the depth those scores are from. Iteratively deepened under a node budget
/// like the normal search, but every root move gets an exact (full-window)
/// score, so it costs several times more per depth.
pub fn analyse_position(
    state: &State,
    max_depth: u32,
    node_budget: u64,
    history: &[u64],
    tt: &dyn Table,
) -> (Vec<Candidate>, u32) {
    tt.bump_age();
    let mut s = Search::new(state, max_depth, history, Some(tt));
    let mut result = (Vec::new(), 0);
    for depth in 1..=max_depth.max(1) {
        s.node_budget = if depth == 1 { u64::MAX } else { node_budget };
        let mut candidates = Vec::new();
        for bit in ordered_moves(s.evaluator, state.empty(), 0, 0) {
            let (child, won) = play_move(s.tables, bit, state);
            let score = if won.is_some() {
                WIN_SCORE + WIN_DEPTH_BONUS * depth as f64
            } else {
                -negamax(&mut s, &child, depth - 1, f64::NEG_INFINITY, f64::INFINITY)
            };
            // Read the line now, before the siblings' searches can overwrite it.
            let line = table_line(s.tt.expect("analysis uses a table"), state, bit, 8);
            candidates.push(Candidate { bit, score, line });
        }
        if s.aborted {
            break;
        }
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        result = (candidates, depth);
        if s.nodes >= node_budget || result.0.first().map_or(true, |c| c.score.abs() >= WIN_SCORE) {
            break;
        }
    }
    result
}

/// One analysed root move: its score for the side to move and the expected
/// continuation, starting with the move itself.
pub struct Candidate {
    pub bit: Bit,
    pub score: f64,
    pub line: Vec<Bit>,
}

/// The continuation the table expects after playing `first` in `state`:
/// follow stored best moves for up to `max_len` plies. Entries may have been
/// overwritten, so the line can be shorter than that; fine for display.
pub fn table_line(tt: &dyn Table, state: &State, first: Bit, max_len: usize) -> Vec<Bit> {
    let t = tables();
    let mut line = vec![first];
    let (mut state, mut won) = play_move(t, first, state);
    while won.is_none() && line.len() < max_len {
        let (key, sym) = canonical_key(t, &state);
        let Some(bit) = tt.get(key).filter(|e| e.best != 0).map(|e| 1 << t.sym_square[t.sym_inverse[sym]][bit_index(e.best)]) else { break };
        if state.occupied() & bit != 0 {
            break; // stale entry from a colliding position
        }
        line.push(bit);
        (state, won) = play_move(t, bit, &state);
    }
    line
}

/// Score of every legal move for the side to move, best first (for analysis).
pub fn root_scores(state: &State, depth: u32, history: &[u64]) -> Vec<(Bit, f64)> {
    let tt = TranspositionTable::new();
    tt.bump_age();
    let mut s = Search::new(state, depth, history, Some(&tt));
    let mut out = Vec::new();
    for bit in ordered_moves(s.evaluator, state.empty(), 0, 0) {
        let (child, won) = play_move(s.tables, bit, state);
        let score = if won.is_some() {
            WIN_SCORE + WIN_DEPTH_BONUS * depth as f64
        } else {
            -negamax(&mut s, &child, depth - 1, f64::NEG_INFINITY, f64::INFINITY)
        };
        out.push((bit, score));
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    out
}

/// Iterative deepening: search depth 1, 2, ... up to `max_depth`, stopping
/// early once `node_budget` nodes have been visited. Each iteration reuses
/// the previous one's transposition table and best move, so the deeper
/// searches start with excellent move ordering.
pub fn best_move_budget(state: &State, max_depth: u32, node_budget: u64, history: &[u64]) -> Option<Bit> {
    let tt = TranspositionTable::new();
    search_depth_reached(state, max_depth, node_budget, history, &tt).0
}

/// As `best_move_budget`, with a table that persists between calls (so the
/// next search starts from what this one learned), and also returning the
/// depth reached and nodes visited.
pub fn search_depth_reached(
    state: &State,
    max_depth: u32,
    node_budget: u64,
    history: &[u64],
    tt: &dyn Table,
) -> (Option<Bit>, u32, u64) {
    let (best, depth, nodes) = best_move_scored(state, max_depth, node_budget, history, tt, true);
    (best.map(|(bit, _)| bit), depth, nodes)
}

/// As `search_depth_reached`, also returning the best move's score for the
/// side to move. With `use_book`, a book move is returned with a score of 0;
/// without it the position is always searched (for evaluations).
pub fn best_move_scored(
    state: &State,
    max_depth: u32,
    node_budget: u64,
    history: &[u64],
    tt: &dyn Table,
    use_book: bool,
) -> (Option<(Bit, f64)>, u32, u64) {
    if use_book {
        if let Some(bit) = book_move(state) {
            return (Some((bit, 0.0)), 0, 0);
        }
    }
    tt.bump_age();
    let mut s = Search::new(state, max_depth, history, Some(tt));
    let mut best = None;
    let mut reached = 0;
    for depth in 1..=max_depth.max(1) {
        let first = best.map_or(0, |(b, _)| b);
        // Depth 1 always completes, so there is always an answer.
        s.node_budget = if depth == 1 { u64::MAX } else { node_budget };
        let result = search_root(&mut s, state, depth, first, 0);
        if s.aborted {
            break; // budget ran out mid-iteration: keep the previous iteration's move
        }
        best = result;
        reached = depth;
        match best {
            None => return (None, reached, s.nodes),               // no legal moves
            Some((_, score)) if score.abs() >= WIN_SCORE => break, // forced result found
            _ if s.nodes >= node_budget => break,
            _ => {}
        }
    }
    (best, reached, s.nodes)
}

/// The move bit the player to move should play, searching exactly `depth`
/// plies (iteratively deepened, no node budget), or None if the board is full.
/// `history` holds the keys of the positions played so far (see `State::key`).
pub fn best_move(state: &State, depth: u32, history: &[u64]) -> Option<Bit> {
    best_move_budget(state, depth, u64::MAX, history)
}

pub fn best_move_index(state: &State, depth: u32, history: &[u64]) -> Option<usize> {
    best_move(state, depth, history).map(bit_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_index_uses_every_slot_range() {
        // Keys spread across the whole table, not just the first 65k slots
        // (the index once shifted by 48 bits, leaving 16 bits, and the mask hid it).
        let mut high = 0;
        for k in 0..100_000u64 {
            high = high.max(table_index(k.wrapping_mul(0x1234_5678_9ABC_DEF1)));
        }
        assert!(high >= TT_SIZE / 2, "highest index seen {high} of {TT_SIZE}");
        assert!(high < TT_SIZE);
    }
}
