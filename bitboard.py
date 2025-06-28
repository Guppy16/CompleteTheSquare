"""Bitboard implementation for CompleteTheSquare game."""

from collections import defaultdict
from dataclasses import dataclass, replace
from itertools import product

DIRECTIONS = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]


@dataclass
class GameConfig:
    players: int = 2
    rows: int = 5
    cols: int = 5


@dataclass(frozen=True)
class BitboardState:
    boards: tuple[int, ...]  # Tuple of bitboards for each player
    current_player: int

    def copy(self):
        """Create a copy of the current state."""
        return BitboardState(self.boards, self.current_player)

    @staticmethod
    def default(players: int = 2) -> "BitboardState":
        """Return a default state with all boards empty and player 0."""
        return BitboardState(boards=tuple(0 for _ in range(players)), current_player=0)


class BitboardGame:
    def __init__(
        self,
        config: GameConfig,
    ):
        self.config = config
        self.move_to_corner_masks = self._get_square_corner_bitmasks()
        self.state = BitboardState.default(config.players)

    def square_to_bitboard(self, square: tuple) -> int:
        """Convert a square's position to a bitboard representation."""
        row, col = square
        assert 0 <= row < self.config.rows
        assert 0 <= col < self.config.cols
        return 1 << (row * self.config.cols + col)

    def show(self):
        """Display the current game state."""
        out = ""
        for row, col in product(range(self.config.rows), range(self.config.cols)):
            if col == 0:
                out += f"\n{row}|"
            position = self.square_to_bitboard((row, col))
            for player in range(self.config.players):
                if self.state.boards[player] & position:
                    out += str(player)
                    break
            else:
                out += "."
        return out

    def is_valid_move(self, move_bitboard: int):
        """Check if a move is valid."""
        return all(
            self.state.boards[player] & move_bitboard == 0
            for player in range(self.config.players)
        )

    def play_move(self, move: tuple):
        """Play a move for a player."""
        move_bitboard = self.square_to_bitboard(move)
        assert self.is_valid_move(move_bitboard)

        player = self.state.current_player
        boards = list(self.state.boards)

        boards[player] = self.state.boards[player] | move_bitboard

        boards = self.remove_pieces(move, boards, player)

        winner = self.get_winner(move_bitboard, player)

        next_player = (player + 1) % self.config.players if not winner else player
        self.state = replace(
            self.state, boards=tuple(boards), current_player=next_player
        )

        return winner

    def _get_square_corner_bitmasks(self):
        """Compute the masks for the corners of all possible squares."""

        move_to_mask: dict[int, list[int]] = defaultdict(list)

        rows, cols = self.config.rows, self.config.cols
        s = min(rows, cols)  # Maximum size of the square

        for size in range(2, s + 1):  # Size of the square (2x2, 3x3, sxs)
            for row_offset in range(rows - size + 1):
                for col_offset in range(cols - size + 1):

                    # Create a bitmask for the corners of the square
                    top_left_mask = self.square_to_bitboard((row_offset, col_offset))
                    top_right_mask = self.square_to_bitboard(
                        (row_offset, col_offset + size - 1)
                    )
                    bottom_left_mask = self.square_to_bitboard(
                        (row_offset + size - 1, col_offset)
                    )
                    bottom_right_mask = self.square_to_bitboard(
                        (row_offset + size - 1, col_offset + size - 1)
                    )

                    # Combine the masks into a single integer
                    mask = (
                        top_left_mask
                        | top_right_mask
                        | bottom_left_mask
                        | bottom_right_mask
                    )

                    # Store the mask
                    move_to_mask[top_left_mask].append(mask)
                    move_to_mask[top_right_mask].append(mask)
                    move_to_mask[bottom_left_mask].append(mask)
                    move_to_mask[bottom_right_mask].append(mask)

        return move_to_mask

    def iter_corner_masks(self):
        """Iterate over all corner masks."""
        for masks in self.move_to_corner_masks.values():
            yield from masks

    def get_winner(self, move_bitboard: int, player: int) -> bool:
        """Determine the winner of the game."""

        return any(
            (self.state.boards[player] & mask) == mask
            for mask in self.move_to_corner_masks[move_bitboard]
        )

    def remove_pieces(self, move: tuple, boards: list[int], player: int):
        """Efficiently remove captured opponent pieces in all directions."""
        row, col = move
        rows, cols = self.config.rows, self.config.cols

        for opponent in set(range(self.config.players)) - {player}:
            for dr, dc in DIRECTIONS:
                captured_mask = 0
                r, c = row + dr, col + dc
                while 0 <= r < rows and 0 <= c < cols:
                    bit = self.square_to_bitboard((r, c))
                    if self.state.boards[opponent] & bit:
                        captured_mask |= bit
                    elif self.state.boards[player] & bit:
                        # Capture! Remove all at once
                        # print(f"Player {player} captures pieces on move {row},{col}")
                        boards[opponent] &= ~captured_mask
                        break
                    else:
                        break
                    r += dr
                    c += dc

        return boards

    def play(self):
        """Play a simple game loop."""
        while True:
            print(self.show())
            move = input(
                f"Player {self.state.current_player}, enter your move (row,col): "
            )
            if move.lower() == "exit":
                break
            try:
                row, col = map(int, move.split(","))
                winner = self.play_move((row, col))
                if winner:
                    print(f"Player {self.state.current_player} wins!")
                    print(self.show())
                    break
            except (ValueError, AssertionError):
                print("Invalid move. Try again.")


def test_simple_game():
    """Test a simple game scenario."""
    config = GameConfig(players=2, rows=5, cols=5)
    game = BitboardGame(config)

    print(game.show())

    # Simulate a few moves
    moves = [(0, 0), (0, 1), (0, 2), (1, 0), (2, 0), (2, 1), (2, 2)]
    for i, move in enumerate(moves):
        winner = game.play_move(move)
        print(game.show())
        if winner:
            print(f"Player {game.state.current_player} wins!")
            break


if __name__ == "__main__":
    test_simple_game()
