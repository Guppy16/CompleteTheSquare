# 9. The transposition table

**Code:** `wasm/src/search.rs`: `TranspositionTable`, `TtEntry`, `Bound`, the probe and
store in `negamax`, `TT_BITS`; `wasm/src/lib.rs`: the `tt` field of `Session`.

## The idea

The same position is reached by many move orders: A1 then B2 for one side and C3 then D4
for the other is the same board as B2 then A1 and D4 then C3. A tree search does not
know this and would search the position again every time. The table is a cache keyed by
the position: score, how deep it was searched, what kind of score it is, and the best
move found.

```rust
struct TtEntry {
    key: u64,      // the canonical position key
    depth: u32,    // plies searched below this position
    score: f64,
    bound: Bound,  // Exact, Lower or Upper
    best: Bit,     // best move, in canonical coordinates
    age: u32,      // which search stored it
}
```

It is a fixed array of `2^18 = 262,144` entries, 8 MB, indexed by a multiplicative hash
of the key:

```rust
fn index(key: u64) -> usize {
    (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - TT_BITS)) as usize
}
```

The constant is 2^64 divided by the golden ratio; multiplying by it and keeping the top
bits scatters similar keys across the table. (An earlier version shifted by 48, leaving
16 bits, so only a quarter of the table was ever used. A unit test now checks the range.)

## The probe

At the top of `negamax`, after the repetition check:

```rust
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
        if alpha >= beta { return e.score; }
    }
}
```

Two uses, in order of value:

1. **Ordering.** Whatever the stored depth, the stored best move is tried first (see
   [Move ordering](07-move-ordering.md)). It is mapped back through the inverse
   symmetry because entries live in the canonical frame (see [Symmetry](10-symmetry.md)).
2. **Reuse.** If the stored search was at least as deep as the one needed now, its score
   can stand in for the whole subtree. An `Exact` score is returned outright. A `Lower`
   bound (the earlier search was cut off at this node, so the true score is at least
   this) can raise alpha; an `Upper` bound can lower beta; if the window closes, return.

## The store

After the loop:

```rust
let bound = if best >= beta { Bound::Lower } else if best <= alpha_in { Bound::Upper } else { Bound::Exact };
if !s.aborted {
    s.tt_put(TtEntry { key: tt_key, depth, score: best, bound, best: best_canonical, age: 0 });
}
```

Nothing is stored from an aborted iteration (see
[Iterative deepening](11-iterative-deepening.md)); those scores are garbage.

## Replacement and ageing

Two positions can hash to the same slot. Which one to keep?

```rust
fn put(&mut self, mut entry: TtEntry) {
    entry.age = self.age;
    let i = Self::index(entry.key);
    if let Some(slot) = self.entries.get_mut(i) {
        if slot.key == entry.key || slot.age != self.age || entry.depth >= slot.depth {
            *slot = entry;
        }
    }
}
```

- Same position: replace (newer is at least as deep).
- Slot from an older search: replace. `Search::new` increments the table's `age`, so
  everything from before the current search counts as old.
- Otherwise keep the deeper of the two.

The first version was "always replace", and it had a visible bug: a 2M-node analysis
overwrote the entries nearest the root before their lines could be displayed, because
millions of shallow leaves flowed through the table after them. Depth-preferred
replacement keeps the valuable entries; ageing stops the table filling with stale deep
entries over a long session.

## Lifetime

The table belongs to the `Session` in `lib.rs` and lives for the whole game (and across
`reset`, since entries describe positions, which stay valid). The positions explored
while choosing one move are exactly where the next search starts, and each pass of the
analysis panel builds on the previous one.

## Worked example

Depth-7 reply to a centre opening, node counts:

| | nodes |
|-|-------|
| killers only | 597,000 |
| with the table keyed by symmetry | 145,000 |

Measured cost of a probe: about one node's worth of work (canonical key plus one array
read). A hit at sufficient depth saves a whole subtree; a miss costs that one node.

## Path-dependent values are stored but never trusted

A position that is a repetition on the current line scores as a draw (see
[Repetition](12-repetition.md)). That value belongs to the line, not the position: the
same position reached by another move order is not a repetition there. Serving it as an
exact score would report a "draw" where none exists, and that can hide a forced win or
invent one. The search counts repetition hits, and a node whose subtree produced any is
stored with a `path_dependent` flag:

```rust
let path_dependent = s.repetition_hits != repetitions_before;
s.tt_put(TtEntry { ..., path_dependent });
// on a probe:
if !e.path_dependent && e.depth >= depth { /* reuse the score or bound */ }
```

The flagged entry's best move is still used, for ordering and for reading lines out of
the table; its score is not. (The first fix skipped the store entirely, which was
correct but left the root's children unstored in this capture-heavy game, since almost
every big subtree contains some repetition, and the expected lines came out one move
long.)

This was found the hard way. Single-threaded, the stale draws were rare enough to pass
as a small error; with sixteen threads sharing one table they were everywhere, and three
searches of the same position gave three different answers, including a false "every
reply loses". Chess engines call this the graph-history interaction problem.
