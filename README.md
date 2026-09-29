# Complete the Square Game

View the [Complete the Square game online](https://Guppy16.github.io/CompleteTheSquare/).

Adapted from: https://github.com/VatsalRaina/CompleteTheSquare


### Development

The page is static: `index.html` plus `square-game/`. Serve it locally with

```bash
python3 -m http.server 3000
```

and open `http://localhost:3000/`.

### The AI

The rules and the AI are a Rust crate in `wasm/`, compiled to WebAssembly and loaded by
`square-game/script.js`. How it works (bitboards, alpha-beta pruning, evaluation) is
explained page by page in [docs/README.md](docs/README.md).

```bash
rustup target add wasm32-unknown-unknown   # once
cd wasm
cargo test --release                       # regression tests
./build.sh                                 # rebuild square-game/ai.wasm
cargo run --release --example analyse -- "1. C3 A1  2. B2 D4"   # what the AI would have played
cargo run --release --example arena -- 10 200                   # win rate vs a human-like opponent (10% blunders, 200 games)
```

The compiled `ai.wasm` is committed, so the site needs no build step to deploy.

### To do

Open items, roughly in order of value.

- **A1 is a first-player win** at depth 16 (every red reply loses; B2 and D4 last longest,
  green's winning square landing on the 16th ply after them; see the
  [opening book page](docs/14-opening-book.md)). Open: whether the other five distinct
  first moves are also wins (at depth 14 their book replies hold), and the winning line
  against B2 written out.
- **Tests for the untested exports** (`analyse`, `evaluate`, `ai_suggest`, `board_at`,
  `repetitions`, the node-budget abort) and a transposition-table-enabled comparison
  against plain search, which would have caught the index bug.
- **Engine headroom, only if wanted:** late move reductions. (Multi-core search exists for
  the offline tools: `analyse -- "<moves>" depth threads`.) The AI already beats a
  blundering depth-5 opponent 96 to 98% of the time.
- **Deferred UI:** a variation tree in the move list (undo/redo covers exploring lines);
  persisting the game across reloads in local storage.
