# 2. Move generation

**Code:** [`wasm/src/game.rs`](../wasm/src/game.rs): `State::empty`; [`wasm/src/search.rs`](../wasm/src/search.rs): `ordered_moves`,
`capture_moves`.

## The idea

A legal move is any empty square. With bitboards the set of legal moves *is* the empty
mask, so generating moves means walking the set bits of one integer instead of looping
over 25 coordinates and testing each.

Two ways to walk set bits appear in the code.

**Filter a fixed order.** The main search wants moves in a particular order (see
[Move ordering](07-move-ordering.md)), so it iterates a precomputed list of all 25 square
bits and keeps the ones that are empty:

```rust
fn ordered_moves(ev: &Evaluator, empty: u32, first: Bit, killer: Bit) -> Vec<Bit> {
    let mut out = Vec::with_capacity(empty.count_ones() as usize);
    if first & empty != 0 { out.push(first); }
    if killer & empty != 0 && killer != first { out.push(killer); }
    out.extend(ev.move_order.iter().copied().filter(|&b| b & empty != 0 && b != first && b != killer));
    out
}
```

**Lowest set bit.** When order does not matter, peel bits off one at a time. This is
what `capture_moves` does:

```rust
let mut empty = state.empty();
while empty != 0 {
    let bit = empty & empty.wrapping_neg();   // isolate the lowest set bit
    empty ^= bit;                             // clear it
    ...
}
```

`x & -x` works because of two's complement: negating flips every bit and adds one, which
leaves exactly the lowest set bit in common with the original.

## Worked example

`empty = 0b101100` (bits 2, 3 and 5 set):

| step | `empty` | `empty & -empty` | after `^=` |
|------|---------|------------------|------------|
| 1 | 0b101100 | 0b000100 (bit 2) | 0b101000 |
| 2 | 0b101000 | 0b001000 (bit 3) | 0b100000 |
| 3 | 0b100000 | 0b100000 (bit 5) | 0 |

Three iterations for three moves; an empty board with 20 free squares costs 20
iterations, not 25.

## Moves stay as bits

The search never converts a move back to `(row, col)`. `play_move` takes the bit,
the transposition table stores the bit, and only the final answer is turned into a square
index with `trailing_zeros()` for the page, which turns it into a grid cell.

---

<sub>← [1. Bitboards](01-bitboards.md) · [index](README.md) · [3. Captures and wins](03-captures-and-wins.md) →</sub>
