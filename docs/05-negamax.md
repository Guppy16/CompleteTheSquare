# 5. Minimax, written as negamax

**Code:** `wasm/src/search.rs`: `negamax` (the loop), `search_root`.

## Minimax

Assume both players play the move that is best for them. Score a position by looking at
every reply, then every reply to that, down to some depth, and evaluate the leaves. At
your own turn take the maximum over children; at the opponent's turn the minimum.

## Negamax

The game is zero-sum: what is good for one side is exactly as bad for the other. So
"the opponent's best" is "the negative of my worst", and the two cases collapse into
one: every node maximises, and scores are negated on the way back up.

The core of `negamax`, with the extras stripped out:

```rust
let mut best = f64::NEG_INFINITY;
for bit in moves {
    let (child, won) = play_move(s.tables, bit, state);
    if won.is_some() {
        best = WIN_SCORE + WIN_DEPTH_BONUS * depth as f64;   // nothing beats winning now
        break;
    }
    let score = -negamax(s, &child, depth - 1, -beta, -alpha);
    if score > best { best = score; }
    ...
}
```

Two conventions make this work:

- every score is for the side to move in that position, so the child's score (for the
  opponent) becomes ours by negation;
- the alpha-beta window is passed as `(-beta, -alpha)`, because the child sees the game
  from the other side (see [Alpha-beta](06-alpha-beta.md)).

There is no `maximizing_player` flag anywhere, and the evaluation is always called for
the side to move. That removes the class of bug the first version of this engine had,
where the heuristic was computed for the wrong player at odd depths.

## Worked example, depth 2

Green to move. Two candidate moves A and B; after each, red has two replies. Leaf
evaluations are for the side to move at the leaf, which is green again:

```
green: A                         green: B
  red: A1 -> leaf +0.10            red: B1 -> leaf +0.30
  red: A2 -> leaf -0.05            red: B2 -> leaf +0.20
```

Red's node under A: children (for green) are +0.10 and -0.05. Negated for red: -0.10
and +0.05. Red maximises: +0.05. Back at green's node, negated: **-0.05** for move A.

Red's node under B: children -0.30 and -0.20 for red; max -0.20; negated: **+0.20** for
move B.

Green picks B. In minimax terms: red chose the reply that minimised green's score under
each move (A2 with -0.05, B2 with +0.20), and green took the maximum of those. Same
answer, one code path.

## Where the root differs

`search_root` is the same loop over the root's moves but keeps the best move as well as
the score, and hands a lost position to [`best_losing_move`](13-lost-positions.md).
