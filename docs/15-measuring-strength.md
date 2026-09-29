# 15. Measuring strength

**Code:** [`wasm/examples/arena/main.rs`](../wasm/examples/arena/main.rs), [`wasm/examples/analyse.rs`](../wasm/examples/analyse.rs);
`search::root_scores`, `search::principal_variation`.

## Only measure

Every evaluation idea that "obviously" should help was tested, and about half of them
lost. The engine got strong through a loop of change, measure, keep or revert.

## The arena: the AI as red against a human-like green

```bash
cd wasm && cargo run --release --example arena -- 10 200        # 10% blunders, 200 games
cargo run --release --example arena -- 10 200 800000            # with a different node budget
```

Green moves first and plays the engine's own depth-5 move, except that on a random
10% of its turns it plays a random legal square. Red is the engine as the page runs it:
iterative deepening under the page's node budget, table kept for the game, book on.
This is the deployed situation, and the number to improve.

Results, 200 games each unless noted:

| engine as red | wins |
|---------------|------|
| plain alpha-beta, fixed depth 7 | 115 |
| plus quiescence (both rows at fixed depth 7) | 195 |
| plus transposition table and node budget | 196 |
| plus symmetry, 400k budget (100 games) | 96 |
| with the bad C3 book entry (40 games) | 14 of 40 |
| C3 entry corrected (100 games) | 96 |

## Why not engine versus engine?

The first attempt was pairwise matches from a set of openings. They turned out to be
almost useless here: from any short opening, the side to move won over 90% of games at
equal strength, whether the openings were random or engine-chosen. This game is decided
by tempo from very early on, so a pairwise match measures who had the move, not which
engine is better. Playing the deployed seat against a blundering opponent is what
correlates with what you see on the phone.

## The analyse tool: what should have been played

```bash
cd wasm && cargo run --release --example analyse -- "1. A1 C3  2. A2 B1  3. A3" 10
```

Paste the text from the page's Copy moves button. For every ply it prints what the
engine would have played at the given depth, then for the final position every legal
move with its exact score (`root_scores`, full-window searches) and the expected line
(`principal_variation`, which re-searches one ply shallower at each step; walking the
table instead is cheaper but the table forgets early entries during a deep search).

Depth 10 takes seconds to a minute; depth 12 several minutes; depth 14 about half an
hour on an idle machine for a near-empty board. Five depth-14 searches run in parallel
took six hours, because the second phase scores every reply with a full window and the
five contended for the same cores. Run deep searches one at a time.

## What the analyses established

- The engine's loss to the A1, A2, A3 column: lost after green's third move at depth 10;
  the losing choice was the reply to A1 (C3), which only shows at depth 14.
- At depth 14 no first move is a forced win for green; at depth 16, A1 is: every red
  reply loses, B2 and D4 last longest (see the [opening book](14-opening-book.md)). The
  other five distinct first moves are open.
- A false version of that result appeared first, from a table that stored repetition
  draws; it was caught because the 16-thread and single-threaded searches disagreed.
- The player to move after any short opening wins over 90% at equal engine strength.

## Benchmarking notes

- Node counts are deterministic; times are not. Compare nodes when the machine is busy.
- 40 games separate large effects (39 vs 14); 100 to 200 games are needed for a few
  percent.
- The search runs at about 6 million nodes per second natively on one core; WebAssembly
  in a browser is perhaps 2 to 4 times slower, a phone slower again.

---

<sub>← [14. The opening book](14-opening-book.md) · [index](README.md) · [16. WebAssembly and the page](16-wasm-and-page.md) →</sub>
