# 4. Evaluation

**Code:** `wasm/src/search.rs`: `Evaluator` (`new`, `evaluate`, `threat_difference`),
the `W_*` and `WIN_*` constants.

## The idea

When the search reaches its depth limit it needs a number for the position. Three
rules shaped the design:

1. **One perspective.** The score is always for the player about to move. The original
   Python version scored from a fixed player's view and applied it at both odd and even
   depths, which flipped the sign at every other level. [Negamax](05-negamax.md) is built
   on the side-to-move convention, so the sign can no longer be wrong.
2. **Wins dominate.** A win found by the search scores `WIN_SCORE` (1.0); the heuristic is
   built so its absolute value never exceeds 0.5. No amount of "nice position" can
   outweigh a forced result.
3. **Sooner is better.** A win found with `d` plies still to go scores `1.0 + 0.01 * d`.
   Among winning lines the search takes the quickest; among losing lines the longest.

## The terms

```rust
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
```

| term | weight | normalised value | meaning |
|------|--------|------------------|---------|
| material | 0.25 | (my pieces - theirs) / 25 | more pieces is better |
| position | 0.10 | corners count 2, edges 1, / 20 | corners cannot be captured (no line through a corner has both sides on the board); edges only along the edge |
| threats | 0.15 | (my threats - theirs), clamped to ±3, / 3 | a threat is a square where I own 3 corners and the 4th is empty |

The weights sum to 0.5 and each term lies in [-1, 1], so the total is inside ±0.5 by
construction.

`threat_difference` walks the 30 corner masks once:

```rust
for &mask in &t.all_corner_masks {
    let (m, th) = (mine & mask, theirs & mask);
    if th == 0 {
        if m.count_ones() == 3 { diff += 1; }
    } else if m == 0 && th.count_ones() == 3 {
        diff -= 1;
    }
}
```

A square with any opponent corner is dead for me, hence `th == 0`.

## Worked example

Green to move with A1, B1, A2 (a corner, two edges); red with C3, D3 (interior).

- material: (3 - 2) / 25 = +0.04, times 0.25 = +0.010
- position: green 2 + 1 + 1 = 4, red 0, (4 - 0) / 20 = 0.2, times 0.10 = +0.020
- threats: green owns three corners of A1 B1 A2 B2 and B2 is empty: +1. Red has none.
  1 / 3 times 0.15 = +0.050
- total: **+0.080** for green.

Had the search seen one more ply it would have found B2 wins outright and returned
1.01 instead, which is the point of keeping the heuristic well below 1.

## What was tried and rejected

A "potential" term for squares with two corners owned and none by the opponent looked
sensible and lost in the arena (27 to 21 at one depth, 19 to 31 at another). Intuition
about evaluation terms was wrong more often than right; see
[Measuring strength](15-measuring-strength.md).

## Knobs

`W_MATERIAL`, `W_POSITION`, `W_THREATS`, `MAX_THREATS` at the top of `search.rs`. Keep the
weights summing to less than `WIN_SCORE`.
