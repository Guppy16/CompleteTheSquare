# How the AI works

The AI is a Rust crate in `wasm/`, compiled to WebAssembly and run in the browser:

- `src/game.rs` is the rules engine: board representation, legal moves, captures, win detection.
- `src/search.rs` is the search: evaluation function, negamax with alpha-beta pruning, killer moves.
- `src/lib.rs` holds the game state and exposes it to the page (section 8).

It started life as Python (`bitboard.py` / `minimax.py`, see the git history), and the
Rust is a line-for-line port, so the snippets below use whichever reads more clearly.

This document walks through each piece, roughly in the order the ideas build on each other.

## 1. Bitboards

A 5x5 board has 25 squares, so the whole set of squares one player occupies fits in a
single 25-bit integer. Square `(row, col)` is bit number `row * 5 + col`:

```
bit index          as a board
 0  1  2  3  4     row 0
 5  6  7  8  9     row 1
10 11 12 13 14     row 2
15 16 17 18 19     row 3
20 21 22 23 24     row 4
```

A game state is just `boards = (player0_bits, player1_bits)` plus whose turn it is.
The nice property is that board questions become single CPU instructions:

| question                         | bit operation                     |
|----------------------------------|-----------------------------------|
| all occupied squares             | `boards[0] | boards[1]`           |
| all empty squares                | `~occupied & full_mask`           |
| is square `s` free               | `occupied & (1 << s) == 0`        |
| does player own all 4 corners of a square | `board & corner_mask == corner_mask` |
| how many pieces does a player have | `board.bit_count()`             |

`full_mask` is `(1 << 25) - 1`, i.e. 25 ones. We need it because `~occupied` in Python
is a negative number with infinitely many leading ones; masking keeps only the 25 real squares.

## 2. Bit-level move generation

The original `legal_moves` looped over all 25 `(row, col)` pairs, built a bit for each one,
and tested it against the occupied mask. The search now asks for the empty mask once and
walks its set bits (in Rust this is `trailing_zeros` plus clearing the bit; in Python it was):

```python
def iter_bits(mask):
    while mask:
        bit = mask & -mask   # isolate the lowest set bit
        yield bit
        mask ^= bit          # clear it and continue
```

`mask & -mask` works because of two's complement: negating a number flips every bit and
adds one, which leaves only the lowest set bit in common with the original. So for
`mask = 0b101100` we get `0b000100`, then `0b001000`, then `0b100000`, and the loop stops
when the mask hits zero. Each iteration costs a couple of integer ops instead of a
coordinate round trip, and an empty board with 20 free squares does 20 iterations, not 25.

The search never converts bits back to coordinates. It plays moves as bits; only the
final answer is translated back to a square index (`best_move_index`), and the page turns
that into a grid cell.

## 3. Captures with precomputed rays

A capture is: starting from the square just played, walk in one of 8 directions over a run
of opponent pieces, and if the run ends at one of our own pieces, remove the run.

The original code recomputed the walk from coordinates on every move, with bounds checks
per visited square. Now `build_tables` runs once at start-up and stores, for every square,
the list of bits along each of the 8 rays:

```
square (2,2), direction "up-right":  [bit(1,3), bit(0,4)]
square (2,2), direction "down":      [bit(3,2), bit(4,2)]
square (0,0), direction "up":        []    (dropped: nothing there)
square (0,0), direction "right":     [bit(0,1), bit(0,2), bit(0,3), bit(0,4)]
```

Rays shorter than two squares are dropped at build time because a capture needs at least
one opponent piece plus one of ours beyond it. At play time `remove_pieces` just walks
the stored bits:

```python
for ray in capture_rays[move_bit]:
    captured = 0
    for bit in ray:
        if opp & bit:    captured |= bit        # extend the run
        elif own & bit:  opp &= ~captured; break  # flanked: remove the run
        else:            break                  # empty: no capture here
```

Win detection was already bit based: `corner_masks[square]` lists every square (2x2 up
to 5x5) that has the played square as a corner, and a win is any mask fully contained in
the player's board.

## 4. Evaluation

Minimax needs a number for positions where the search has run out of depth. Three rules
shaped the design:

1. **Fixed perspective.** The score is always "for the player about to move". The old
   `piece_fraction` heuristic did this too, but the old search treated every value as being
   from the AI's point of view, so at odd depths the sign was wrong. Negamax (section 5)
   is built around the "player to move" convention, so this is now consistent.
2. **Wins must dominate.** A real win or loss found by the search scores `+-1.0`; the
   heuristic is capped at `+-0.5` by construction. No pile of positional bonuses can ever
   outweigh a forced loss.
3. **Sooner wins score higher.** A win found with `d` plies of depth still remaining
   scores `1.0 + 0.01 * d`. That makes the AI finish the game instead of shuffling when it
   has several winning lines, and drag a lost game out as long as possible.

The heuristic is a weighted sum of three terms, each normalised to `[-1, 1]`:

| term       | weight | what it measures                                                     |
|------------|--------|----------------------------------------------------------------------|
| material   | 0.25   | `(my pieces - their pieces) / 25`                                    |
| position   | 0.10   | corners count 2, edges count 1, normalised by the board's total       |
| threats    | 0.15   | squares where one side owns 3 corners and the 4th corner is free, difference clamped to +-3 |

Corners and edges are weighted because of the capture rule: a corner piece can never be
captured (every line through it leaves the board on one side), and an edge piece can only
be captured along the edge. They are the safest material on the board.

## 5. Minimax, written as negamax

Minimax is "assume both sides play the move that is best for them". At an AI node we take
the maximum over child scores, at an opponent node the minimum.

Negamax is the same algorithm with one trick: because the game is zero-sum, "the opponent's
best score" is just the negative of "my worst score". So every node maximises, and we
negate on the way back up:

```python
def negamax(state, depth):
    if depth == 0: return evaluate(state)          # for the player to move
    best = -inf
    for move in legal_moves(state):
        child, won = play(move, state)
        if won: return WIN                          # can't do better than winning now
        best = max(best, -negamax(child, depth - 1))
    return best
```

There is no `maximizing_player` flag and no chance of using the wrong sign, which is
exactly the class of bug the old heuristic had.

## 6. Alpha-beta pruning

Plain minimax visits every node: with about 20 legal moves and depth 5 that is
20^5 = 3.2 million leaf evaluations. Alpha-beta gets the identical answer while skipping
most of them.

Carry two bounds down the tree:

- `alpha`: the best score the player to move is already guaranteed somewhere else in the tree.
- `beta`: the best score the opponent is already guaranteed somewhere else.

At any node, if we find a move scoring `>= beta`, we can stop looking at this node's other
moves. Why: the opponent already has a line elsewhere worth `beta` to them; this position
gives them something worse, so they will never let the game reach it. Whatever the
remaining moves score cannot change the answer at the root. That early `break` is the
**cut-off**.

Worked example, from the AI's point of view, depth 2:

```
root (AI to move)
 ├─ move A ─ opponent's replies score  0.10, 0.05, 0.20  → opponent picks 0.05
 └─ move B ─ opponent's first reply scores 0.02 ...
```

After move A the AI knows it can guarantee `alpha = 0.05`. Searching move B, the very
first opponent reply gives only `0.02`. The opponent picks the minimum, so move B is worth
at most `0.02 < 0.05`. There is no need to look at the opponent's other replies to B: the
AI will play A regardless. The subtree under B is pruned.

In negamax form the window is passed as `(-beta, -alpha)` to the child, because the child
sees the game from the other side. The code in `negamax` is:

```python
score = -negamax(child, depth - 1, -beta, -alpha)
best = max(best, score)
alpha = max(alpha, best)
if alpha >= beta: break      # cut-off
```

`alpha_beta_matches_plain_minimax` in `wasm/tests/ai.rs` checks on random positions that
the pruned search returns exactly the same value as the unpruned one.

### Move ordering matters

Cut-offs only happen when a good move is tried *before* the weaker ones. With perfect
ordering alpha-beta visits about `2 * b^(d/2)` leaves instead of `b^d`; with bad ordering
it degrades back to plain minimax. Two cheap tricks give most of the benefit:

- **Corners first.** `Evaluator.move_order` is the 25 squares sorted by distance from the
  nearest corner. This matches what the evaluation rewards, so strong moves are tried
  early. It is also the tie-break between equally scored moves, which is why the AI opens
  in a corner rather than the centre. (Centre-first ordering was tried first: it visited
  about 4x more nodes at depth 6 and made the AI open in the centre.)
- **Killer moves.** `killers[depth]` remembers the last move that caused a cut-off at that
  depth. Positions that are siblings in the tree are very similar, so the move that refuted
  one usually refutes the next. It is tried first.

Measured on the opening position (AI replying to a centre move), depth 5:

| ordering               | nodes visited | time  |
|------------------------|---------------|-------|
| centre-first only      | 336,000       | 2.5 s |
| centre-first + killers | 60,000        | 0.4 s |
| corner-first + killers | 35,000        | 0.25 s |

### Quiescence: don't evaluate mid-exchange

A fixed-depth search stops at arbitrary moments. If the last move searched was a capture,
the static evaluation counts the captured piece as gone even when the very next move takes
one back, and the search chases these phantom gains ("the horizon effect"). This game is
full of capture exchanges, so it mattered a lot: at equal depth the fix won 36 of 50 games
against the version without it.

`quiescence` runs at every leaf instead of `evaluate`. The side to move may "stand pat"
(take the static evaluation) or play any *capturing* move, and the search continues down
captures only, for at most `QUIESCENCE_DEPTH` more plies. Two shortcuts: a leaf where the
side to move already has three corners of a square with the fourth empty is scored as a
win, and a capture that completes a square is a win.

### Transposition table and iterative deepening

The same position is reached by many move orders. `Search.tt` is a fixed-size table (65k
entries) keyed by `State::key` that remembers the score, the depth it was searched to, and
the best move found. On a hit deep enough, the score is reused (with care: a score found
during a cut-off is only a bound, see `Bound`). On any hit, the stored best move is tried
first, which is better move ordering than the killer heuristic alone.

The table key is the *canonical* form of the position: the smallest key among its 16
equivalents, the 8 rotations and reflections of the board, each with and without the
colours swapped (swapping colours and the side to move leaves the value for the side to
move unchanged). A position and its mirror image therefore share one entry, and the stored
best move is mapped back through the inverse symmetry on a hit. Cost: about 80 table
lookups per node. Gain: the depth-7 reply to a centre opening dropped from 597k nodes to
145k. The table lives in the `Session` for the whole game, so each search starts from what
the previous one learned.

`best_move_budget` searches depth 1, then 2, then 3, and so on, keeping the table between
iterations, until a node budget is used up or a forced win is found. Each iteration starts
with the previous one's best move, so the deeper searches prune very well, and the cost of
the shallow iterations is negligible. The page asks for a budget of 400k nodes and a
maximum depth of 12; that is depth 7 or 8 in about 100 ms on a laptop.

## 7. Repetition

Captures let positions recur: one side blocks a threat by capturing, the other recaptures
and renews the threat, and neither can deviate without losing. Left alone the search
happily walks this loop, because each recapture looks like "gain a piece and threaten".

Two rules handle it, borrowed from chess:

- **In the search**, `Search.path` holds the keys of every position in the game so far plus
  the current line. A position already in it can only lead to a draw, so `negamax` returns
  a draw score instead of searching it. The draw is worth `-DRAW_CONTEMPT` to the AI and
  `+DRAW_CONTEMPT` to the opponent, so the AI only repeats when every alternative scores
  worse than that (it is losing anyway), and it expects the opponent to repeat whenever
  that suits them.
- **In the game**, `lib.rs` keeps the same history; when a position occurs for the third
  time the game ends as a draw (`draw()` returns 1). The page warns after the second
  occurrence.

`State::key` packs both boards and the side to move into one `u64` so the check is a scan
of a few dozen integers per node; it did not measurably change search time.

## 8. Measuring strength

Only measure; intuition about evaluation terms was wrong more often than right (a
"two corners owned" potential term looked sensible and lost). Two tools in `wasm/examples`:

- `cargo run --release --example arena -- human` plays the AI as red against a human-like
  green (engine moves with a percentage of random blunders) over hundreds of games and
  prints the win rate. That is the deployed situation. Pairwise engine-vs-engine matches
  turned out to be nearly useless here: from any short opening, the side to move wins over
  90% of games at equal strength, so results measure the seat, not the engine.
- `cargo run --release --example analyse -- "1. C3 A1  2. ..."` replays a game pasted from
  the page's "Copy moves" button and prints, ply by ply, what the engine would have played
  instead. Use it on games the AI lost.

## 9. Tuning knobs

- `AI_NODE_BUDGET` and `AI_MAX_DEPTH` in `square-game/script.js`. The search runs at about
  6 million nodes per second natively; a phone in WebAssembly is perhaps 3 to 5x slower.
  The budget is checked at every node: when it runs out the current iteration is abandoned
  and the previous iteration's move is played, so the time per move is predictable.
  Measured against the human-like opponent, 200k nodes won 94 of 100, 400k won 97, and
  800k also 97, so 400k is the setting.
- `QUIESCENCE_DEPTH` in `src/search.rs` (4): how far capture exchanges are followed at the
  leaves.
- The three weights and `WIN_SCORE` at the top of `src/search.rs`. Keep the weights
  summing to less than `WIN_SCORE` so wins always dominate.
- `Evaluator::move_order` if you want to experiment with other static orderings.
- `DRAW_CONTEMPT` in `src/search.rs` (currently 0.2, about 20 pieces of material or one
  and a half threats): how much worse than a draw a position must be before the AI will
  repeat.

## 10. The WebAssembly build

`src/lib.rs` keeps one game in a thread-local `Session` and exports plain-integer
functions, so the page needs no glue library:

| export            | meaning                                                            |
|-------------------|--------------------------------------------------------------------|
| `reset()`         | new game, player 0 to move                                         |
| `board(p)`        | bitboard of player `p`'s pieces                                    |
| `current_player()`| 0 or 1                                                             |
| `winner()`        | winner's index, or -1 while the game runs                          |
| `winning_mask()`  | corner mask of the completed square, for highlighting              |
| `draw()`          | 1 once the game is drawn by threefold repetition                   |
| `repetitions()`   | how many times the current position has occurred                   |
| `play(row, col)`  | 1 if the move was applied, 0 if illegal or the game is over        |
| `ai_play(max_depth, node_budget)` | choose and play a move for the side to move; returns its square index |
| `ai_suggest(max_depth, node_budget)` | the same search without playing the move (Hint)          |
| `analyse(max_depth, node_budget)` | score every legal move for the side to move; read with `analysis_move(i)`, `analysis_score(i)`, `analysis_line(i, j)` (expected continuation), `analysis_depth()` |

The page's Analysis tab calls `analyse` repeatedly with a doubling node budget (0.5M up to
8M nodes, a few seconds in the browser) so the shown depth keeps rising while the position
stands still; a "Go deeper" button continues for a few more seconds, the way lichess's
engine panel works. The worker keeps its table between calls, so each deeper pass starts
from the previous one's work. It shows the top three lines with their expected
continuations (read from the table right after each candidate is searched, before its
siblings can overwrite them), scores from Green's point of view, an eval bar, markers on the
board whose opacity fades with the gap to the best move, and a score after every move of
the game in the move list (`evaluate`, a quick search per position).

`square-game/script.js` fetches `ai.wasm`, calls `WebAssembly.instantiate`, and from then
on only forwards clicks to `play` and redraws the grid from `board(0)` and `board(1)`.

The search itself runs in a Web Worker (`square-game/worker.js`) so the page never
freezes while the AI thinks. The worker holds a second copy of the module: the page sends
it the moves played so far, the worker replays them and answers with `ai_suggest`, and the
page plays that square on its own copy. Answers that arrive after the game has moved on
(undo, new game) are dropped. `wasm/build.sh` rebuilds the module; the compiled file is
committed so GitHub Pages can serve it with no build step.
