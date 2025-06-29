"""Minimax algorithm for Complete The Square game."""

from statistics import mean, median
from typing import Literal

from bitboard import BitboardGame, BitboardState, GameConfig


def heuristic(
    state: BitboardState, heuristic_type: Literal["simple", "piece_fraction"]
) -> float:
    """Heuristic evaluation function for the game state."""

    if heuristic_type == "simple":
        # Simple heuristic: return 0.0 for simplicity
        return 0.0
    elif heuristic_type == "piece_fraction":
        # Piece count difference heuristic
        piece_counts = [bitboard.bit_count() for bitboard in state.boards]
        player_pieces = piece_counts[state.current_player]
        total_pieces = sum(piece_counts)

        return player_pieces / total_pieces if total_pieces > 0 else 0.0


def minimax(
    game: BitboardGame,
    state: BitboardState,
    depth: int,
    maximizing_player: bool,
    visited: set[BitboardState],
) -> float:

    if state in visited:
        return -0.1  # Already visited state, return draw value
    visited.add(state)

    if depth == 0:
        visited.remove(state)
        return heuristic(state, "simple")

    legal_moves = game.legal_moves(state)
    if not legal_moves:
        visited.remove(state)
        return 0.0

    if maximizing_player:
        value = float("-inf")
        win_value = 1.0
        value_operation = max
    else:
        value = float("inf")
        win_value = -1.0
        value_operation = min

    for move in legal_moves:
        new_state, winner = game.play_move(move, state)

        if winner:
            visited.remove(state)
            return win_value  # Return winning value for the current player
        score = minimax(game, new_state, depth - 1, not maximizing_player, visited)
        value = value_operation(value, score)

    visited.remove(state)
    return value  # Return draw value if no winning move found


def find_best_move(
    game: BitboardGame,
    state: BitboardState,
    depth: int,
) -> tuple[int, int]:

    best_value = float("-inf")
    best_move = None

    for move in game.legal_moves(state):
        new_state, winner = game.play_move(move, state)

        if winner:
            return move  # Immediate winning move

        visited = set()
        score = minimax(game, new_state, depth - 1, False, visited)

        if score > best_value:
            best_value = score
            best_move = move

    return (
        best_move if best_move else (0, 0)
    )  # Return a default move if no moves available


def play_minimax_game(
    game: BitboardGame,
    depth: int = 3,
):
    """Play a game using the minimax algorithm."""
    state = game.new_game_state()

    while True:
        print(game.show(state))
        if state.current_player == 0:  # Player 0 is human
            move = input(f"Player {state.current_player}, enter your move (row,col): ")
            if move.lower() == "exit":
                break
            try:
                row, col = map(int, move.split(","))
                move = (row, col)
                state, winner = game.play_move(move, state)
            except Exception as e:
                print(f"Invalid move: {e}")
                continue
        else:  # Player 1 is AI
            move = find_best_move(game, state, depth)
            state, winner = game.play_move(move, state)
            print(f"AI played move: {move}")

        if winner:
            print(f"Player {state.current_player} wins!")
            break


if __name__ == "__main__":
    config = GameConfig(players=2, rows=5, cols=5)
    game = BitboardGame(config)
    play_minimax_game(game, depth=3)
