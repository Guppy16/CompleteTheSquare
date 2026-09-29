# 17. Parallel search (native tools only)

**Code:** `wasm/src/parallel.rs`: `SharedTable`, `best_move_parallel`,
`root_scores_parallel`, `principal_variation_parallel`; `wasm/src/search.rs`: the `Table`
trait, `Search::with_evaluator`, `Search::with_stop`; `wasm/tests/parallel.rs`.

## Scope

The browser build stays single-threaded: WebAssembly threads need cross-origin headers
GitHub Pages cannot set, and the page's search already takes a fraction of a second. The
parallel code is for the offline tools, where a depth-14 search of one opening takes
half an hour on one core and a 16-core machine was sitting idle. It is compiled only for
native targets (`#[cfg(not(target_arch = "wasm32"))]`).

## The seam: a `Table` trait

The one thing threads must share is the transposition table, and the single-threaded
one is a plain `Vec` behind `Cell`s. So the search now talks to a trait:

```rust
pub trait Table {
    fn get(&self, key: u64) -> Option<TtEntry>;
    fn put(&self, entry: TtEntry);          // &self: a shared table is interior-mutable
    fn age(&self) -> u32;
    fn bump_age(&self);
}
```

`Search` holds `Option<&dyn Table>`, plus an optional evaluator (so each thread can
use its own move order) and an optional stop flag checked where the node budget is
checked. Nothing else in the search changed, which is the point: the parallel driver
runs the existing search several times.

## The shared table, lock-free

Each slot is two atomic 64-bit words. The entry is packed into one word (score as
`f32` bits, depth, bound, best-move square, age) and the other word holds the key
XOR-ed with it:

```rust
fn get(&self, key: u64) -> Option<TtEntry> {
    let w0 = slot[0].load(Relaxed);
    let w1 = slot[1].load(Relaxed);
    (w0 ^ w1 == key && w1 != 0).then(|| unpack(key, w1))   // torn write => mismatch => miss
}
```

Two threads storing to the same slot at once can interleave their two words. Then
`w0 ^ w1` is not the key of either entry, the read is a miss, and nothing wrong is ever
used. No locks, no waiting, and the cost of the rare collision is one lost entry. The
same replacement policy as the single-threaded table applies (`should_replace`).

## Lazy SMP: `best_move_parallel`

Every thread runs the same iterative deepening on the same position over the shared
table. To stop them doing identical work, odd-numbered threads start one ply deeper,
and thread `i` uses the static move order rotated by `i`. They exchange discoveries
through the table: when thread 3 finishes searching a subtree, thread 0 finds the
result there and skips it. The first thread to complete the target depth, or to find a
forced result, sets the stop flag; the others abort their current iteration (nothing
half-finished is stored), and the deepest completed result wins.

It sounds too simple to work. It is what Stockfish does, and the reason it works is
that the table *is* the search's memory: sharing it shares the work.

## Root splitting: `root_scores_parallel`

Scoring every root move with a full window, the slow second phase of the analyse tool,
is embarrassingly parallel: deal the moves out round-robin and let each thread score its
share over the shared table. Scores are exact either way, so the result is the same as
single-threaded up to the `f32` rounding of table entries.

## Tests

- **Torn reads.** Two threads hammer eight slots with 200,000 conflicting writes each;
  every read must be a miss or a self-consistent entry with the right key.
- **Agreement.** On a dozen random positions, parallel root scores at 2 and 4 threads
  equal the single-threaded scores per move within 1e-4. (Moves with tied scores may
  come out in a different order; the first version of the test wrongly demanded the
  same order and failed every run, which is the kind of "race" that is really a test bug.)
- **Legality and depth.** The parallel best move is legal, reaches the requested depth,
  and with one thread reproduces the serial move exactly.

All run five times in a row before the code was committed, since a race that shows up
one run in ten would pass a single run.

## Use

```bash
cd wasm && cargo run --release --example analyse -- "1. A1" 14 16   # 16 threads
```

The single-threaded path is the default (`threads` = 1) and is what the tests compare
against.
