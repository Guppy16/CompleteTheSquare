# How the AI works

The rules engine and the AI are a Rust crate in `wasm/`, compiled to WebAssembly and run
in the browser. Each page below covers one idea, shows the code that implements it, and
works through a small example. Read them in order the first time; afterwards each one
stands alone.

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
| 14 | [Opening book](14-opening-book.md) | precomputed replies to the first move |
| 15 | [Measuring strength](15-measuring-strength.md) | the arena and the analyse tool, and what they showed |
| 16 | [WebAssembly and the page](16-wasm-and-page.md) | exports, the worker, the analysis tab |
| 17 | [Parallel search](17-parallel-search.md) | lazy SMP and root splitting for the offline tools |

## Where the code is

| file | holds |
|------|-------|
| `wasm/src/game.rs` | `State`, the precomputed `Tables`, `play_move`, captures, win detection, symmetry |
| `wasm/src/search.rs` | `Evaluator`, `negamax`, `quiescence`, `TranspositionTable`, iterative deepening, book, analysis |
| `wasm/src/lib.rs` | the `Session` (move list, undo/redo) and every `extern "C"` export |
| `wasm/src/parallel.rs` | native-only: the shared table, lazy SMP, root splitting |
| `wasm/tests/ai.rs` | the regression tests |
| `wasm/examples/arena` | the AI as red against a human-like opponent |
| `wasm/examples/analyse.rs` | replay a game; every move scored; expected line |
| `square-game/script.js`, `worker.js` | the page and the worker that runs the search |

Line numbers move, so the pages name functions rather than lines. To jump to one:

```bash
grep -n "fn negamax" wasm/src/search.rs
```

## Glossary

- **ply**: one player's move. A "move" in the numbered list is a pair of plies.
- **node**: one position visited by the search.
- **depth**: how many plies ahead the search looks from the current position.
- **horizon**: the depth at which the search stops and asks the evaluation instead.
- **score**: always for the side to move, in the range -1 (lost) to +1 (won). The page
  shows scores for Green, like lichess shows them for White.
