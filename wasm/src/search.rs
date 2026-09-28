//! Evaluation and negamax alpha-beta search. A direct port of `minimax.py`.

use crate::game::{bit_index, index_to_square, play_move, tables, Bit, State, Tables, COLS, N, ROWS};
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

/// Legal move bits: the killer first, then corner-out order.
fn ordered_moves<'a>(ev: &'a Evaluator, empty: u32, killer: Bit) -> impl Iterator<Item = Bit> + 'a {
    let first = if killer & empty != 0 { Some(killer) } else { None };
    first.into_iter().chain(ev.move_order.iter().copied().filter(move |&b| b & empty != 0 && b != killer))
}

/// Everything one search needs besides the position.
pub struct Search {
    pub tables: &'static Tables,
    pub evaluator: &'static Evaluator,
    /// The player the search is for (draw scores are relative to them).
    pub root_player: usize,
    /// Keys of every position so far: the game's history, then the current line.
    pub path: Vec<u64>,
    /// killers[depth] = the move that last caused a cut-off at that depth.
    pub killers: Vec<Bit>,
}

impl Search {
    pub fn new(root: &State, depth: u32, history: &[u64]) -> Self {
        let mut path = history.to_vec();
        if path.last() != Some(&root.key()) {
            path.push(root.key());
        }
        Search { tables: tables(), evaluator: evaluator(), root_player: root.current, path, killers: vec![0; depth as usize + 1] }
    }

    /// Score of a draw for the player to move in `state`.
    fn draw_score(&self, state: &State) -> f64 {
        if state.current == self.root_player { -DRAW_CONTEMPT } else { DRAW_CONTEMPT }
    }
}

pub fn negamax(s: &mut Search, state: &State, depth: u32, mut alpha: f64, beta: f64) -> f64 {
    // A position we have already been through can only lead to a draw by
    // repetition, whatever the evaluation says about it.
    if s.path.contains(&state.key()) {
        return s.draw_score(state);
    }
    if depth == 0 {
        return s.evaluator.evaluate(s.tables, state);
    }
    let empty = state.empty();
    if empty == 0 {
        return s.draw_score(state);
    }

    s.path.push(state.key());
    let mut best = f64::NEG_INFINITY;
    let moves: Vec<Bit> = ordered_moves(s.evaluator, empty, s.killers[depth as usize]).collect();
    for bit in moves {
        let (child, won) = play_move(s.tables, bit, state);
        if won.is_some() {
            best = WIN_SCORE + WIN_DEPTH_BONUS * depth as f64;
            break;
        }
        let score = -negamax(s, &child, depth - 1, -beta, -alpha);
        if score > best {
            best = score;
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
    best
}

/// The move bit the player to move should play, or None if the board is full.
/// `history` holds the keys of the positions played so far (see `State::key`).
pub fn best_move(state: &State, depth: u32, history: &[u64]) -> Option<Bit> {
    let mut s = Search::new(state, depth, history);
    let empty = state.empty();
    let mut best_bit = None;
    let mut best_score = f64::NEG_INFINITY;
    let (mut alpha, beta) = (f64::NEG_INFINITY, f64::INFINITY);

    let moves: Vec<Bit> = ordered_moves(s.evaluator, empty, 0).collect();
    for bit in moves {
        let (child, won) = play_move(s.tables, bit, state);
        if won.is_some() {
            return Some(bit);
        }
        let score = -negamax(&mut s, &child, depth - 1, -beta, -alpha);
        if score > best_score {
            best_score = score;
            best_bit = Some(bit);
        }
        alpha = alpha.max(best_score);
    }
    best_bit
}

pub fn best_move_index(state: &State, depth: u32, history: &[u64]) -> Option<usize> {
    best_move(state, depth, history).map(bit_index)
}
