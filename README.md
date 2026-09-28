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
explained in [docs/ai-search.md](docs/ai-search.md).

```bash
rustup target add wasm32-unknown-unknown   # once
cd wasm
cargo test --release                       # regression tests
./build.sh                                 # rebuild square-game/ai.wasm
cargo run --release --example analyse -- "1. C3 A1  2. B2 D4"   # what the AI would have played
cargo run --release --example arena -- human                    # win rate vs a human-like opponent
```

The compiled `ai.wasm` is committed, so the site needs no build step to deploy.
