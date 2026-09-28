"""Print random positions with the Python engine's answers, for tests/parity.rs.

The Rust test runs this script and checks its own engine gives the same
answers. One line per case:

    b0 b1 player  move_row move_col  new_b0 new_b1 new_player won  eval  best_row best_col

where (b0, b1, player) is a position, (move_row, move_col) a random legal move,
(new_b0, new_b1, new_player, won) the result of playing it, eval the heuristic
score of the position, and (best_row, best_col) the depth-4 search's choice.
"""

import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", ".."))
from bitboard import BitboardGame, GameConfig  # noqa: E402
from minimax import Evaluator, find_best_move  # noqa: E402

DEPTH = 4
CASES = 300

game = BitboardGame(GameConfig(players=2, rows=5, cols=5))
ev = Evaluator(game)
rng = random.Random(2024)
lines = []
while len(lines) < CASES:
    state = game.new_game_state()
    for _ in range(rng.randrange(0, 20)):
        state, won = game.play_move(rng.choice(game.legal_moves(state)), state)
        if won:
            break
    else:
        move = rng.choice(game.legal_moves(state))
        new_state, won = game.play_move(move, state)
        best = find_best_move(game, state, DEPTH, ev)
        lines.append(
            f"{state.boards[0]} {state.boards[1]} {state.current_player} "
            f"{move[0]} {move[1]} "
            f"{new_state.boards[0]} {new_state.boards[1]} {new_state.current_player} {int(won)} "
            f"{ev.evaluate(state)!r} {best[0]} {best[1]}"
        )

print("\n".join(lines))
