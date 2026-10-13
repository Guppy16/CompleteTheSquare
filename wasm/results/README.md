# Rotated squares: does the first move still win?

Under the variant (`--features rotated-squares`) tilted squares win too: 50 winning squares
instead of 30. Run on 12 threads, the opening book off for both rule sets.

## Search

`first_moves -- 14 12` scores Green's six distinct first moves at each depth: the full
tables are in `rotated-first-moves.txt` and `standard-first-moves.txt`. Depth 14, scores for
Green (+1.000 is a forced win):

| rules | A1 | B1 | C1 | B2 | C2 | C3 | time |
|---|---|---|---|---|---|---|---|
| rotated | **+1.000** | -0.010 | +0.030 | -0.060 | -0.140 | **+1.000** | 50 min |
| standard | +0.270 | -0.050 | -0.090 | -0.015 | -0.125 | -0.020 | 14 min |

Under the variant, A1 and C3 are forced wins for Green within 14 plies. Under the standard
rules no first move is (A1 is a win only at depth 16).

## Self-play

`calibrate -- 4000 12 400000 5 openings.txt`: the engine against itself at the page's
budget, 5% random moves, 667 games from each first move. Green's score:

| rules | A1 | B1 | C1 | B2 | C2 | C3 | all | draws | moves |
|---|---|---|---|---|---|---|---|---|---|
| rotated | 0.94 | 0.74 | 0.81 | 0.22 | 0.13 | 0.94 | 0.63 | 0.3% | 13.9 |
| standard | 0.90 | 0.49 | 0.26 | 0.80 | 0.17 | 0.21 | 0.47 | 4.6% | 16.9 |

The engine repeats itself, so each opening's number reflects a few lines, not the position.

## In short

The first move's advantage grows: two forced wins at depth 14 instead of none, and Green
scores 0.63 in self-play instead of 0.47. Games are shorter and draws all but disappear.
Counting squares alone (the central cells sit on the most) predicts C2 and C3 as best and
A1 as worst: A1 is in fact a forced win, since a corner piece can never be captured, and C2
is Green's worst first move.
