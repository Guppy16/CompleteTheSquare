# 11. Iterative deepening and the node budget

**Code:** [`wasm/src/search.rs`](../wasm/src/search.rs): `best_move_scored`, `search_root`, the `node_budget`
and `aborted` fields of `Search` and their checks in `negamax`;
[`square-game/script.js`](../square-game/script.js): `AI_NODE_BUDGET`, `AI_MAX_DEPTH`.

## Iterative deepening

Search to depth 1, then 2, then 3, and so on, reusing the transposition table between
rounds:

```rust
for depth in 1..=max_depth.max(1) {
    let first = best.map_or(0, |(b, _)| b);
    s.node_budget = if depth == 1 { u64::MAX } else { node_budget };   // depth 1 always completes
    let result = search_root(&mut s, state, depth, first);
    if s.aborted {
        break;                    // budget ran out mid-iteration: keep the previous move
    }
    best = result;
    reached = depth;
    match best {
        None => return (None, reached, s.nodes),                // no legal moves
        Some((_, score)) if score.abs() >= WIN_SCORE => break,  // forced result found
        _ if s.nodes >= node_budget => break,
        _ => {}
    }
}
```

It sounds wasteful and isn't. Each round starts with the previous round's best move at
the root and a table full of best moves for the positions below, so the deeper search
prunes far better than a cold search to the same depth would. The shallow rounds cost a
small fraction of the last one; the cost of depth 1..d-1 together is roughly the cost of
depth d divided by the branching factor.

## The node budget

A fixed depth costs wildly different amounts in different positions: depth 7 on the open
board is a hundred times more work than depth 7 near the end of a game. So the page
asks for a budget of positions rather than a depth. `negamax` counts nodes and sets a
flag when the budget is spent:

```rust
s.nodes += 1;
if s.nodes >= s.node_budget {
    s.aborted = true;     // out of budget: unwind; the caller discards this iteration
}
if s.aborted {
    return 0.0;
}
```

Once set, every call returns immediately, the tree unwinds in microseconds, nothing
from the abandoned iteration is stored in the table, and the loop above plays the move
from the last completed iteration. Time per move is therefore predictable: the page's
400k nodes take about 70 ms on a laptop and a few hundred ms on a phone, and the search
reaches whatever depth that buys, deeper in the endgame, shallower in the opening.

The budget used to be checked only between iterations, which let the last iteration
overshoot by several times, and with a persistent table (which makes early iterations
nearly free) the overshoot got worse. Checking per node fixed the timing; the budget was
raised from 200k to 400k to keep the strength, which the measurements below justify.

## Worked example: choosing the budget

The AI as red against a depth-5 opponent that blunders 10% of the time, 100 games each:

| budget | wins | avg nodes per move | time per 100 games |
|--------|------|--------------------|--------------------|
| 200k | 94 | 107k | 30 s |
| 400k | 97 | 207k | 92 s |
| 800k | 97 | 394k | 224 s |

Average nodes are below the budget because many searches end early on a forced result.

## Knobs

- `AI_NODE_BUDGET` and `AI_MAX_DEPTH` in [`square-game/script.js`](../square-game/script.js) for play.
- `ANALYSIS_FIRST_BUDGET`, `ANALYSIS_BUDGET`, `ANALYSIS_MAX_DEPTH` for the analysis tab,
  which doubles the budget pass after pass (see [WebAssembly and the page](16-wasm-and-page.md)).

---

<sub>← [10. Symmetry](10-symmetry.md) · [index](README.md) · [12. Repetition](12-repetition.md) →</sub>
