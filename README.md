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
cargo run --release --example arena -- 10 200                   # win rate vs a human-like opponent (10% blunders, 200 games)
```

The compiled `ai.wasm` is committed, so the site needs no build step to deploy.

### To do

Open items, roughly in order of value.

- **Click through the Analysis tab in a browser.** Nothing built on 29 Sep has been run in
  a browser by the tooling: check the lines fill in and the depth climbs, "Go deeper"
  appears after a few seconds, per-move scores populate in the move list, tapping a line
  plays it, the panel stays put, and the eval bar and markers fit on a phone.
- **Opening book entry for C2.** The one first move without a depth-14 reply. About 30 to
  45 minutes on an idle machine:
  `cd wasm && cargo run --release --example analyse -- "1. C2" 14`, then validate the
  candidate with the arena before adding it to `OPENING_BOOK` in `wasm/src/search.rs`
  (the C3 entry showed a depth-14 score can be wrong in play).
- **GCP:** the Cloud Build trigger and the Cloud Run service are deleted. Still to do:
  disconnect the repository under Cloud Build → Repositories, and remove the Google Cloud
  Build app from the repo on GitHub (Settings → Integrations) so no check appears on
  commits. Check Billing → Reports a day later.
- **Git remote over SSH** so pushes need no token:
  `git remote set-url origin git@github.com:Guppy16/CompleteTheSquare.git`.
- **Is A1 a forced win?** Not at depth 14 (B2 and D4 hold). A definitive answer needs a
  solver rather than a depth-limited search; with the symmetry-keyed table this is plausible
  as an overnight run.
- **Tests for the untested exports** (`analyse`, `evaluate`, `ai_suggest`, `board_at`,
  `repetitions`, the node-budget abort) and a transposition-table-enabled comparison
  against plain search, which would have caught the index bug.
- **Engine headroom, only if wanted:** late move reductions, and a multi-core split. The
  AI already beats a blundering depth-5 opponent 96 to 98% of the time.
- **Deferred UI:** a variation tree in the move list (undo/redo covers exploring lines);
  persisting the game across reloads in local storage.
