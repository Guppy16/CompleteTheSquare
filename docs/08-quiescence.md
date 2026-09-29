# 8. Quiescence search

**Code:** `wasm/src/search.rs`: `quiescence`, `capture_moves`, `Evaluator::has_threat`,
`QUIESCENCE_DEPTH`.

## The problem: the horizon effect

A fixed-depth search stops at an arbitrary moment. If the last move inside the horizon
was a capture, the evaluation counts the captured piece as gone even when the very next
move takes one back. The search then chases these phantom gains, or fears phantom
losses. This game is full of capture exchanges, so it hurt badly: the version without
quiescence won 115 of 200 games against a blundering opponent; with it, 195.

## The fix

At a leaf, instead of calling `evaluate` directly, call `quiescence`. It keeps
searching, but only capturing moves, until the position is quiet:

```rust
pub fn quiescence(s: &mut Search, state: &State, mut alpha: f64, beta: f64, qdepth: u32) -> f64 {
    let (mine, theirs) = (state.boards[state.current], state.boards[1 - state.current]);
    if ev.has_threat(t, mine, theirs) {
        return WIN_SCORE;                  // we complete a square next move
    }
    let stand_pat = ev.evaluate(t, state);
    if qdepth == 0 || stand_pat >= beta {
        return stand_pat;
    }
    if stand_pat > alpha { alpha = stand_pat; }
    let mut best = stand_pat;
    for bit in capture_moves(t, state) {
        let (child, won) = play_move(t, bit, state);
        if won.is_some() { return WIN_SCORE; }
        let score = -quiescence(s, &child, -beta, -alpha, qdepth - 1);
        if score > best { best = score; }
        if best > alpha { alpha = best; }
        if alpha >= beta { break; }
    }
    best
}
```

Three ideas in there:

- **Stand pat.** The side to move may decline to capture and take the static evaluation.
  Captures can only improve on it, so `best` starts at `stand_pat`. This is what makes
  the result a fair estimate rather than "assume a capture is forced".
- **Captures only.** `capture_moves` walks the empty squares and keeps those where some
  ray has an opponent run closed by our own piece, using the same precomputed rays as
  the rules engine. The tree is narrow because captures are rare.
- **A threat is a win.** If the side to move already owns three corners of a square with
  the fourth empty, it wins next move whatever else happens, so the leaf scores as a
  win without searching.

`QUIESCENCE_DEPTH` (4) caps how far the exchange is followed.

## Worked example

Depth runs out right after red captured green's C2 by playing D2 (green has B2):

```
   A B C D E
2  . G . R .      green to move, C2 now empty
```

Static evaluation: red is a piece up, say -0.03 for green. But green can play C2 and
capture D2 back through E2 if green has E2. Quiescence tries that capture: after it,
green is level again, and the exchange is over; it returns about +0.01. The leaf is
scored +0.01, not -0.03, and the search stops seeing red's capture as a gain.

## Cost

Every leaf now costs at least an evaluation plus a capture scan. At equal depth the
search is about 1.5x slower and far stronger; with a node budget it simply reaches a
slightly lower depth per move, which the measurements show is a good trade.
