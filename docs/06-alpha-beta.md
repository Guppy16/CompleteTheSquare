# 6. Alpha-beta pruning

**Code:** `wasm/src/search.rs`: `negamax` (the `alpha`/`beta` handling), the
`alpha_beta_matches_plain_minimax` test in `wasm/tests/ai.rs`.

## The idea

Plain minimax visits every node. With about 20 legal moves and depth 5 that is
20^5 = 3.2 million leaves. Alpha-beta gives the identical answer while skipping most of
them.

Carry two bounds down the tree:

- `alpha`: the best score the side to move is already guaranteed somewhere else.
- `beta`: the best score the opponent is already guaranteed somewhere else.

At any node, if a move scores `>= beta`, stop looking at this node's other moves. The
opponent already has a line elsewhere worth `beta` to them; this node gives them
something worse, so they will never let the game reach it. Whatever the remaining moves
score cannot change the answer at the root. That early exit is the **cut-off**.

In negamax form:

```rust
let score = -negamax(s, &child, depth - 1, -beta, -alpha);
if score > best { best = score; best_bit = bit; }
if best > alpha { alpha = best; }
if alpha >= beta {
    s.killers[depth as usize] = bit;     // remember what cut off (see Move ordering)
    break;                               // cut-off
}
```

The window `(alpha, beta)` becomes `(-beta, -alpha)` for the child because the child
scores from the other side's point of view.

## Worked example

Depth 2, green to move, same tree as in [Negamax](05-negamax.md) but searched with
bounds. Root window is (-inf, +inf).

1. Move A. Red's replies score +0.10 and -0.05 for green, so red picks -0.05. Green now
   knows: "I can get at least -0.05". `alpha = -0.05`.
2. Move B, searched with the window (-inf, -0.05) from red's side, i.e. red must beat
   -(-0.05) = +0.05 to matter. Red's first reply scores +0.30 for green, which is -0.30
   for red, worse than the +0.05 red needs... no cut-off yet. Red's second reply scores
   +0.20 for green, -0.20 for red. Red's best is -0.20, still below +0.05: B is worth
   +0.20 to green, better than A. `alpha = +0.20`.

Now suppose there were a move C whose first reply gave green only +0.02. Red's best
under C is at least -0.02 for red, i.e. green gets at most +0.02 from C. Green already
has +0.20 from B, so C is skipped after one reply. That is the cut-off: the remaining
replies under C are never generated.

## Fail-soft and bounds

The code returns `best` even when it is outside the window ("fail-soft"). The returned
value is then not the exact score but a bound:

- `best >= beta`: the true score is at least `best` (a lower bound; the search was cut off).
- `best <= alpha_in`: the true score is at most `best` (an upper bound; nothing reached alpha).
- otherwise exact.

The [transposition table](09-transposition-table.md) records which of the three it was.

## Why it gives the same answer

The pruned branches could only have produced scores the parent would reject anyway. The
test `alpha_beta_matches_plain_minimax` checks this on random positions: a plain
full-width search and the pruned search return exactly the same value, to 1e-12.

## How much it saves

With the best move always tried first, alpha-beta visits about `2 * b^(d/2)` leaves
instead of `b^d`: depth 6 costs about what depth 3 cost before. With bad ordering it
degrades back towards plain minimax, which is why [Move ordering](07-move-ordering.md)
matters so much.
