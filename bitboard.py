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

    @property
    def occupied(self) -> int:
        """Bitmask of all occupied squares (OR of all boards)."""
        result = 0
        for board in self.boards:
            result |= board
        return result

    def copy(self):
        """Create a copy of the current state."""
        return BitboardState(self.boards, self.current_player)

    @staticmethod
    def default(players: int = 2) -> "BitboardState":
        """Return a default state with all boards empty and player 0."""
        return BitboardState(boards=tuple(0 for _ in range(players)), current_player=0)

    def is_valid_move(self, move_bitboard: int) -> bool:
        """Check if a move is valid."""
        return self.occupied & move_bitboard == 0


class BitboardGame:
    def __init__(
        self,
        config: GameConfig,
    ):
        self.config = config
        self.move_to_corner_masks = self._get_square_corner_bitmasks()

    def new_game_state(self) -> BitboardState:
        """Create a new game state with all boards empty and player 0."""
        return BitboardState.default(self.config.players)

    def square_to_bitboard(self, square: tuple) -> int:
        """Convert a square's position to a bitboard representation."""
        row, col = square
        assert 0 <= row < self.config.rows
        assert 0 <= col < self.config.cols
        return 1 << (row * self.config.cols + col)

    def show(self, state: BitboardState):
        """Display the current game state."""
        out = ""
        for row, col in product(range(self.config.rows), range(self.config.cols)):
            if col == 0:
                out += f"\n{row}|"
            position = self.square_to_bitboard((row, col))
            for player in range(self.config.players):
                if state.boards[player] & position:
                    out += str(player)
                    break
            else:
                out += "."
        return out

    def legal_moves(self, state: BitboardState) -> list[tuple]:
        """Generate all legal moves for the current player."""

        moves = []
        for row, col in product(range(self.config.rows), range(self.config.cols)):
            position = self.square_to_bitboard((row, col))
            if state.is_valid_move(position):
                moves.append((row, col))
        return moves

    def play_move(self, move: tuple, state: BitboardState):
        """Play a move for a player."""
        move_bitboard = self.square_to_bitboard(move)
        assert state.is_valid_move(move_bitboard)

        player = state.current_player
        boards = list(state.boards)

        # Update the board for the current player
        boards[player] |= move_bitboard
        # Check if the move captures any opponent pieces
        boards = self.remove_pieces(move, boards, player)

        winner = self.get_winner(move_bitboard, player, boards[player])

        next_player = (player + 1) % self.config.players if not winner else player
        new_state = replace(state, boards=tuple(boards), current_player=next_player)

        return new_state, winner

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

    def get_winner(self, move_bitboard: int, player: int, player_board: int) -> bool:
        """Determine the winner of the game."""

        return any(
            (player_board & mask) == mask
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
                    if boards[opponent] & bit:
                        captured_mask |= bit
                    elif boards[player] & bit:
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

        state = self.new_game_state()

        while True:
            print(self.show(state))
            move = input(f"Player {state.current_player}, enter your move (row,col): ")
            if move.lower() == "exit":
                break
            try:
                row, col = map(int, move.split(","))
                state, winner = self.play_move((row, col), state)
                if winner:
                    print(f"Player {state.current_player} wins!")
                    print(self.show(state))
                    break
            except (ValueError, AssertionError):
                print("Invalid move. Try again.")


def test_simple_game():
    """Test a simple game scenario."""
    config = GameConfig(players=2, rows=5, cols=5)
    game = BitboardGame(config)
    state = game.new_game_state()

    print(game.show(state))

    # Simulate a few moves
    moves = [(0, 0), (0, 1), (0, 2), (1, 0), (2, 0), (2, 1), (2, 2)]
    for i, move in enumerate(moves):
        state, winner = game.play_move(move, state)
        print(game.show(state))
        if winner:
            print(f"Player {state.current_player} wins!")
            break


def test_board_state_hash():
    """Test the hashability of BitboardState."""
    state1 = BitboardState.default(2)
    state2 = BitboardState.default(2)

    assert hash(state1) == hash(state2), "States should be hashable and equal"

    # Modify state1
    state1 = replace(state1, current_player=1)
    assert hash(state1) != hash(state2), "Modified state should have a different hash"


if __name__ == "__main__":
    test_board_state_hash()
    test_simple_game()
