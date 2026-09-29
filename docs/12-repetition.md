# 12. Repetition

**Code:** `wasm/src/search.rs`: the `path` field of `Search`, `Search::draw_score`,
`DRAW_CONTEMPT`, the check at the top of `negamax`; `wasm/src/lib.rs`: `Session::keys`,
`Session::repetitions`, `Session::draw`, `REPETITION_LIMIT`.

## The problem

Captures let positions recur. A real game showed the loop: red owned three corners of
a 2x2 square and threatened to complete it; green blocked at C2, which captured red's D2
through green's E2; red replayed D2, which captured C2 through red's B2 and restored the
threat. Each recapture looked to the search like "gain a piece and threaten", so both
sides would have repeated forever.

## In the search: any repeat is a draw

`Search.path` holds the keys of every position in the game so far plus the current line
of the search. `negamax` pushes the node's key before its loop and pops it after, so the
vector is a stack of the line being explored. Before anything else:

```rust
if s.path.contains(&key) {
    return s.draw_score(state);
}
```

A position already seen can only lead to a draw by repetition, whatever the evaluation
says about it, so it is scored as one without being searched. A single repeat is enough
here (the game needs three): if a position recurs once, the side that repeated can
repeat again, so the line can never do better than a draw.

## Contempt

A draw is not worth zero to the AI:

```rust
pub const DRAW_CONTEMPT: f64 = 0.2;

fn draw_score(&self, state: &State) -> f64 {
    if state.current == self.root_player { -DRAW_CONTEMPT } else { DRAW_CONTEMPT }
}
```

A repetition scores -0.2 for the AI and +0.2 for the opponent. So the AI repeats only
when every alternative looks worse than -0.2, which with the evaluation capped at ±0.5
means "clearly losing", and it expects the opponent to repeat whenever that suits them.
Chess engines call this contempt; 0.2 is about twenty pieces of material or one and a
half threats.

## In the game: threefold

`Session` keeps every ply, so the history is the list of position keys from the empty
board onwards. After each move, if the new position has now occurred `REPETITION_LIMIT`
(3) times, `draw()` becomes true and the game is over. The page shows "Draw by threefold
repetition" and warns after the second occurrence.

## Worked example

The loop above, with the AI as red, from the position after green's block:

- Red's search considers D2 (recapture). The child position is the one before green's
  block, which is in the game history, so it scores -0.2 immediately.
- Any other red move is searched normally. If one of them scores above -0.2, red plays
  it and the loop is broken.
- If red were actually losing everywhere else (all below -0.2), it would take the
  repetition, which is the right call: a draw beats a loss.

A test in `wasm/tests/ai.rs` replays this exact position and checks both that the AI
declines the repetition and that the game declares the threefold draw.

## Cost

The check is a scan of a few dozen `u64`s per node (game length plus search depth). It
did not measurably change search time.
