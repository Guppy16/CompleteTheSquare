# 16. WebAssembly and the page

**Code:** [`wasm/src/lib.rs`](../wasm/src/lib.rs) (the `Session` and every export), [`wasm/build.sh`](../wasm/build.sh),
[`square-game/script.js`](../square-game/script.js), [`square-game/worker.js`](../square-game/worker.js).

## The module owns the game

`lib.rs` keeps one game in a thread-local `Session`: the list of plies played (each with
the position after it and whether it won), a redo stack, the transposition table, and
the last analysis. Everything else, the winner, the draw, the repetition count, is
derived from the ply list, which is why undo and redo are a pop and a push.

Every export takes and returns plain integers, so the page needs no glue library beyond
`WebAssembly.instantiate`:

| export | meaning |
|--------|---------|
| `reset()` | new game, player 0 to move (the table is kept) |
| `board(p)` | bitboard of player `p`'s pieces |
| `board_at(ply, p)` | the same after the first `ply` moves (for the move-list pictures) |
| `current_player()` | 0 or 1 |
| `winner()` | winner's index, or -1 while the game runs |
| `winning_mask()` | corner mask of the completed square, for highlighting |
| `draw()` | 1 once the game is drawn by threefold repetition |
| `repetitions()` | how many times the current position has occurred |
| `play(row, col)` | 1 if the move was applied, 0 if illegal or the game is over |
| `undo()` / `redo()` | take back / replay one move; 1 if something happened |
| `move_count()`, `move_at(i)`, `redo_count()` | the move list and the redo stack |
| `ai_suggest(max_depth, node_budget)` | the square the engine would play |
| `ai_play(max_depth, node_budget)` | as above but plays it too (kept for scripts) |
| `analyse(max_depth, node_budget)` | score every legal move; read with `analysis_move(i)`, `analysis_score(i)`, `analysis_line(i, j)`, `analysis_depth()` |
| `evaluate(max_depth, node_budget)` | the best move's score for the side to move |
| `book_square()` | the opening book's move for the position, or -1 |

[`wasm/build.sh`](../wasm/build.sh) compiles with `cargo build --release --target wasm32-unknown-unknown`
and copies the 66 KB result to [`square-game/ai.wasm`](../square-game/ai.wasm), which is committed so GitHub
Pages serves the site with no build step. The release profile uses `opt-level = 3`,
LTO, `panic = "abort"` and `strip`.

## The page is a renderer

`script.js` loads the module, forwards clicks to `play`, and redraws the 25 cells from
`board(0)` and `board(1)` after every change. Mode is one of `board` (two humans),
`ai`, or `analysis`; switching keeps the game so a finished game can be analysed.

## The worker

The search runs in a Web Worker so the page never freezes. `worker.js` holds a second
copy of the module. The page sends the moves played so far; the worker replays them
into its own session, runs the request, and posts the answer back. Requests carry an id
so replies match up, and every reply is checked against a snapshot of the move list
taken when the request was sent: if the game moved on meanwhile (undo, new game, a tab
switch) the answer is dropped.

Because the worker keeps its own session, its transposition table persists across
requests, which is what makes the analysis passes and the next move's search build on
earlier work.

If the worker fails to start or dies, the page's own copy of the module answers the
outstanding requests and the worker is not used again, so a broken worker degrades to a
blocking search rather than a frozen game.

## The Analysis tab

Modelled on lichess's engine panel:

- `analyse` scores every legal move for the side to move with an exact full-window
  search per move, iteratively deepened under a budget. The page starts at 0.5M nodes
  and re-runs with a doubling budget up to 8M (a few seconds in the browser), then
  offers "Go deeper" for three more doublings. Each pass replaces the display; the depth
  shown climbs.
- The top three lines are shown with their expected continuation, read from the table
  right after each candidate is searched (before its siblings can overwrite it), written
  with move numbers as lichess does. The book move, when there is one, is listed first
  and tagged.
- Scores are shown for Green, as lichess shows them for White: the side-to-move score
  is negated when Red is to move.
- The eval bar shows Green's share; a forced result fills it.
- Markers on the board show the candidates, shrinking and fading with their gap to the
  best move.
- `evaluate` scores every position of the game with a quick search, one at a time in
  the worker, cached by move prefix, and the move list shows the score after each move.

## Memory

Each copy of the module starts with about 1 MB of linear memory and grows by 8 MB for
the table on first use. Page plus worker is roughly 20 MB, a fraction of what the tab
itself costs.

---

<sub>← [15. Measuring strength](15-measuring-strength.md) · [index](README.md) · [17. Parallel search (native tools only)](17-parallel-search.md) →</sub>
