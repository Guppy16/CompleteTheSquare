"""Regression tests for the rules engine and the AI.  Run: python3 test_ai.py"""

import random

from bitboard import BitboardGame, BitboardState, GameConfig
from minimax import Evaluator, find_best_move, negamax

game = BitboardGame(GameConfig(players=2, rows=5, cols=5))
evaluator = Evaluator(game)


def position(p0, p1, current_player):
    boards = tuple(sum(game.square_to_bitboard(sq) for sq in pieces) for pieces in (p0, p1))
    return BitboardState(boards, current_player)


def squares(board):
    return {game.index_to_square[b.bit_length() - 1] for b in game.iter_bits(board)}


def test_captures():
    # Player 1 plays (2,0); its run (2,1),(2,2) is flanked by player 0 at (2,3) -> captured.
    # The diagonal run (1,1) is not flanked (nothing at (0,2)) -> stays.
    state = position(p0=[(2, 3)], p1=[(2, 1), (2, 2), (1, 1)], current_player=0)
    state, won = game.play_move((2, 0), state)
    assert not won
    assert squares(state.boards[1]) == {(1, 1)}
    assert squares(state.boards[0]) == {(2, 0), (2, 3)}


def test_alpha_beta_matches_plain_minimax():
    """Pruning and move ordering must never change the value of a position."""

    def plain_negamax(state, depth):
        if depth == 0:
            return evaluator.evaluate(state)
        empty = game.empty_mask(state)
        if not empty:
            return 0.0
        best = float("-inf")
        for bit in game.iter_bits(empty):
            new_state, won = game.play_move_bit(bit, state)
            if won:
                return 1.0 + 0.01 * depth
            best = max(best, -plain_negamax(new_state, depth - 1))
        return best

    rng = random.Random(0)
    for _ in range(30):
        state = game.new_game_state()
        for _ in range(rng.randrange(0, 12)):
            state, won = game.play_move(rng.choice(game.legal_moves(state)), state)
            if won:
                break
        else:
            expected = plain_negamax(state, 3)
            killers = [0] * 4
            actual = negamax(game, evaluator, state, 3, float("-inf"), float("inf"), killers)
            assert abs(expected - actual) < 1e-12, (state, expected, actual)


def test_ai_behaviour():
    # Takes an immediate win.
    assert find_best_move(game, position([(0, 0), (0, 2), (2, 0)], [(4, 4), (4, 3), (3, 4)], 1), 5, evaluator) == (3, 3)
    # Blocks an immediate threat.
    assert find_best_move(game, position([(0, 0), (0, 2), (2, 0)], [(4, 4)], 1), 5, evaluator) == (2, 2)
    # Replies to a centre opening with a corner (the evaluation and move
    # ordering both prefer corners: they can never be captured).
    reply = find_best_move(game, position([(2, 2)], [], 1), 5, evaluator)
    assert reply in {(0, 0), (0, 4), (4, 0), (4, 4)}, reply


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print(f"{name}: ok")
