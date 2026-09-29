//! Rules engine: bitboard representation, legal moves, captures, win detection.
//! Originally a port of a Python prototype; see `docs/README.md` for the ideas.

use std::sync::OnceLock;

pub const ROWS: usize = 5;
pub const COLS: usize = 5;
pub const N: usize = ROWS * COLS;
pub const PLAYERS: usize = 2;
/// Every square set: turns "not occupied" into "empty".
pub const FULL_MASK: u32 = (1u32 << N) - 1;

const DIRECTIONS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

pub type Bit = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State {
    pub boards: [u32; PLAYERS],
    pub current: usize,
}

impl State {
    pub const fn new() -> Self {
        State { boards: [0; PLAYERS], current: 0 }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn occupied(&self) -> u32 {
        self.boards[0] | self.boards[1]
    }
    pub fn empty(&self) -> u32 {
        !self.occupied() & FULL_MASK
    }
    /// Packs the position and side to move into one integer, for repetition checks.
    pub fn key(&self) -> u64 {
        (self.boards[0] as u64) | ((self.boards[1] as u64) << N) | ((self.current as u64) << (2 * N))
    }
}

pub const fn square_bit(row: usize, col: usize) -> Bit {
    1 << (row * COLS + col)
}
pub fn bit_index(bit: Bit) -> usize {
    bit.trailing_zeros() as usize
}
pub fn index_to_square(i: usize) -> (usize, usize) {
    (i / COLS, i % COLS)
}

/// Tables computed once at start-up.
pub struct Tables {
    /// For each square: the corner masks of every square it is a corner of.
    pub corner_masks: Vec<Vec<u32>>,
    /// Every distinct corner mask on the board (sorted).
    pub all_corner_masks: Vec<u32>,
    /// For each square: rays of bits walking away in each direction (len >= 2 only).
    pub capture_rays: Vec<Vec<Vec<Bit>>>,
    /// Board symmetries (4 rotations, 4 reflections). sym_square[s][i] is
    /// where square i lands under symmetry s; sym_rows[s][r][bits] is the
    /// transformed bitboard of row r holding `bits`, so a whole board is
    /// transformed with 5 lookups.
    pub sym_square: Vec<[usize; N]>,
    pub sym_rows: Vec<[[u32; 32]; ROWS]>,
    /// sym_inverse[s] undoes symmetry s.
    pub sym_inverse: [usize; SYMMETRIES],
}

pub const SYMMETRIES: usize = 8;

/// Where (row, col) lands under symmetry `s` on a square board.
fn symmetry(s: usize, r: usize, c: usize) -> (usize, usize) {
    let n = ROWS - 1;
    match s {
        0 => (r, c),
        1 => (c, n - r),         // rotate 90
        2 => (n - r, n - c),     // rotate 180
        3 => (n - c, r),         // rotate 270
        4 => (r, n - c),         // mirror left-right
        5 => (n - r, c),         // mirror top-bottom
        6 => (c, r),             // transpose
        _ => (n - c, n - r),     // anti-transpose
    }
}

/// `board` seen through symmetry `s`.
pub fn transform(t: &Tables, s: usize, board: u32) -> u32 {
    let rows = &t.sym_rows[s];
    let mut out = 0;
    for (r, row) in rows.iter().enumerate() {
        out |= row[(board >> (r * COLS)) as usize & 0b11111];
    }
    out
}

/// The smallest key among the 16 equivalent positions (8 board symmetries,
/// with and without the colours swapped), and the symmetry that produced it.
/// Swapping colours and the side to move leaves the position's value for the
/// side to move unchanged, so all 16 can share one table entry.
pub fn canonical_key(t: &Tables, state: &State) -> (u64, usize) {
    let mut best = (u64::MAX, 0);
    for s in 0..SYMMETRIES {
        let (a, b) = (transform(t, s, state.boards[0]), transform(t, s, state.boards[1]));
        let same = State { boards: [a, b], current: state.current }.key();
        let swapped = State { boards: [b, a], current: 1 - state.current }.key();
        let k = same.min(swapped);
        if k < best.0 {
            best = (k, s);
        }
    }
    best
}

pub fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(build_tables)
}

fn build_tables() -> Tables {
    let mut corner_masks = vec![Vec::new(); N];
    let s = ROWS.min(COLS);
    for size in 2..=s {
        for r in 0..=(ROWS - size) {
            for c in 0..=(COLS - size) {
                let corners = [
                    square_bit(r, c),
                    square_bit(r, c + size - 1),
                    square_bit(r + size - 1, c),
                    square_bit(r + size - 1, c + size - 1),
                ];
                let mask = corners.iter().fold(0, |m, b| m | b);
                for b in corners {
                    corner_masks[bit_index(b)].push(mask);
                }
            }
        }
    }
    let mut all_corner_masks: Vec<u32> = corner_masks.iter().flatten().copied().collect();
    all_corner_masks.sort_unstable();
    all_corner_masks.dedup();

    let mut capture_rays = vec![Vec::new(); N];
    for i in 0..N {
        let (row, col) = index_to_square(i);
        for (dr, dc) in DIRECTIONS {
            let mut ray = Vec::new();
            let (mut r, mut c) = (row as i32 + dr, col as i32 + dc);
            while r >= 0 && r < ROWS as i32 && c >= 0 && c < COLS as i32 {
                ray.push(square_bit(r as usize, c as usize));
                r += dr;
                c += dc;
            }
            if ray.len() >= 2 {
                capture_rays[i].push(ray);
            }
        }
    }
    // Symmetry tables (the board must be square for rotations to make sense).
    assert_eq!(ROWS, COLS);
    let mut sym_square = Vec::with_capacity(SYMMETRIES);
    let mut sym_rows = Vec::with_capacity(SYMMETRIES);
    for s in 0..SYMMETRIES {
        let mut squares = [0; N];
        for (i, sq) in squares.iter_mut().enumerate() {
            let (r, c) = index_to_square(i);
            let (r2, c2) = symmetry(s, r, c);
            *sq = r2 * COLS + c2;
        }
        let mut rows = [[0u32; 32]; ROWS];
        for (r, row) in rows.iter_mut().enumerate() {
            for (bits, out) in row.iter_mut().enumerate() {
                for c in 0..COLS {
                    if bits & (1 << c) != 0 {
                        *out |= 1 << squares[r * COLS + c];
                    }
                }
            }
        }
        sym_square.push(squares);
        sym_rows.push(rows);
    }
    let mut sym_inverse = [0; SYMMETRIES];
    for (s, inv) in sym_inverse.iter_mut().enumerate() {
        *inv = (0..SYMMETRIES)
            .find(|&j| (0..N).all(|i| sym_square[j][sym_square[s][i]] == i))
            .expect("every symmetry has an inverse");
    }
    Tables { corner_masks, all_corner_masks, capture_rays, sym_square, sym_rows, sym_inverse }
}

/// Remove opponent runs flanked between the played square and one of ours.
fn remove_pieces(t: &Tables, move_bit: Bit, boards: &mut [u32; PLAYERS], player: usize) {
    let own = boards[player];
    for opponent in 0..PLAYERS {
        if opponent == player {
            continue;
        }
        let mut opp = boards[opponent];
        for ray in &t.capture_rays[bit_index(move_bit)] {
            let mut captured = 0;
            for &bit in ray {
                if opp & bit != 0 {
                    captured |= bit;
                } else if own & bit != 0 {
                    opp &= !captured;
                    break;
                } else {
                    break;
                }
            }
        }
        boards[opponent] = opp;
    }
}

/// The corner mask of a completed square containing `move_bit`, if any.
pub fn winning_mask(t: &Tables, move_bit: Bit, board: u32) -> Option<u32> {
    t.corner_masks[bit_index(move_bit)].iter().copied().find(|&m| board & m == m)
}

/// Play `move_bit` for the current player. Returns the new state and the
/// winning square's mask if the move won.
pub fn play_move(t: &Tables, move_bit: Bit, state: &State) -> (State, Option<u32>) {
    debug_assert!(state.occupied() & move_bit == 0);
    let player = state.current;
    let mut boards = state.boards;
    boards[player] |= move_bit;
    remove_pieces(t, move_bit, &mut boards, player);
    let won = winning_mask(t, move_bit, boards[player]);
    let next = if won.is_some() { player } else { (player + 1) % PLAYERS };
    (State { boards, current: next }, won)
}
