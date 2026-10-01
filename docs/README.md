# How the AI works

The rules engine and the AI are a Rust crate in [`wasm/`](../wasm/), compiled to
WebAssembly and run in the browser. Each page below covers one idea, shows the code that
implements it, and works through an example on a real board. Read them in order the first
time; afterwards each one stands alone.

<p align="center"><img src="img/game.svg" alt="an animated game" width="340"></p>

> [!NOTE]
> **Glossary**
>
> | term | meaning |
> |------|---------|
> | **ply** | one player's move. A numbered "move" in the move list is a pair of plies, one per side. |
> | **node** | one position visited by the search. Speeds and budgets are counted in nodes. |
> | **depth** | how many plies ahead the search looks from the current position. |
> | **horizon** | the depth at which the search stops and asks the evaluation instead of looking further. |
> | **score** | how good a position is for the **side to move**, from -1 (lost) to +1 (won); anything strictly between ±0.5 is a heuristic guess. The page shows scores for Green, as lichess shows them for White. |
> | **forced win** | a line where every defence loses; scores just above +1 (a sooner win scores higher). |
> | **square names** | columns A–E left to right, rows 1–5 top to bottom. Green moves first. |

## The pages

| # | page | one line |
|---|------|----------|
| 1 | [Bitboards](01-bitboards.md) | a player's pieces as one 25-bit integer |
| 2 | [Move generation](02-move-generation.md) | legal moves are the set bits of the empty mask |
| 3 | [Captures and wins](03-captures-and-wins.md) | precomputed rays and corner masks |
| 4 | [Evaluation](04-evaluation.md) | scoring a position when the search stops |
| 5 | [Negamax](05-negamax.md) | minimax with one sign convention |
| 6 | [Alpha-beta pruning](06-alpha-beta.md) | skipping branches that cannot matter |
| 7 | [Move ordering](07-move-ordering.md) | corner-first, killer moves, table moves |
| 8 | [Quiescence](08-quiescence.md) | never evaluate in the middle of a capture exchange |
| 9 | [Transposition table](09-transposition-table.md) | remembering positions already searched |
| 10 | [Symmetry](10-symmetry.md) | sixteen positions that share one table entry |
| 11 | [Iterative deepening and the node budget](11-iterative-deepening.md) | predictable time per move |
| 12 | [Repetition](12-repetition.md) | draws by repetition in the search and in the game |
| 13 | [Lost positions](13-lost-positions.md) | picking a move when every move loses |
| 14 | [Opening book](14-opening-book.md) | precomputed replies to the first move; A1 is a win |
| 15 | [Measuring strength](15-measuring-strength.md) | the arena and the analyse tool, and what they showed |
| 16 | [WebAssembly and the page](16-wasm-and-page.md) | exports, the worker, the analysis tab |
| 17 | [Parallel search](17-parallel-search.md) | lazy SMP and root splitting for the offline tools |

## Where the code is

| file | holds |
|------|-------|
| [`wasm/src/game.rs`](../wasm/src/game.rs) | `State`, the precomputed `Tables`, `play_move`, captures, win detection, symmetry |
| [`wasm/src/search.rs`](../wasm/src/search.rs) | `Evaluator`, `negamax`, `quiescence`, the transposition table, iterative deepening, book, analysis |
| [`wasm/src/parallel.rs`](../wasm/src/parallel.rs) | native only: the shared table, lazy SMP, root splitting |
| [`wasm/src/lib.rs`](../wasm/src/lib.rs) | the `Session` (move list, undo/redo) and every `extern "C"` export |
| [`wasm/tests/`](../wasm/tests/) | regression tests; parallel-versus-serial agreement |
| [`wasm/examples/`](../wasm/examples/) | arena, analyse, experiments, the diagram generator |
| [`square-game/script.js`](../square-game/script.js), [`worker.js`](../square-game/worker.js) | the page, and the worker that runs the search |

The pages name functions rather than line numbers, which move. To jump to one:

```bash
grep -n "fn negamax" wasm/src/search.rs
```

The pictures are SVG files in [`img/`](img/), drawn from real positions by the engine:
boards with [`diagram.rs`](../wasm/examples/diagram.rs), search trees with
[`tree_diagram.rs`](../wasm/examples/tree_diagram.rs), the animated game with
[`animate_game.rs`](../wasm/examples/animate_game.rs).

## Further reading

The techniques here are standard in game-playing programs, mostly from chess. Good
starting points:

- [Minimax](https://en.wikipedia.org/wiki/Minimax), [negamax](https://en.wikipedia.org/wiki/Negamax), [alpha-beta pruning](https://en.wikipedia.org/wiki/Alpha%E2%80%93beta_pruning)
- [Bitboards](https://en.wikipedia.org/wiki/Bitboard)
- [Horizon effect](https://en.wikipedia.org/wiki/Horizon_effect) and [quiescence search](https://en.wikipedia.org/wiki/Quiescence_search)
- [Transposition table](https://en.wikipedia.org/wiki/Transposition_table), [killer heuristic](https://en.wikipedia.org/wiki/Killer_heuristic), [iterative deepening](https://en.wikipedia.org/wiki/Iterative_deepening_depth-first_search)
- The [Chess Programming Wiki](https://www.chessprogramming.org/Main_Page), especially [Lazy SMP](https://www.chessprogramming.org/Lazy_SMP), [contempt](https://www.chessprogramming.org/Contempt_Factor) and the [graph history interaction](https://www.chessprogramming.org/Graph_History_Interaction) problem behind page 9's repetition fix
