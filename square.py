from dataclasses import dataclass
from itertools import product


@dataclass
class SquareConfig:
    players = 2
    rows = 5
    cols = 5

    def compute_square_corner_masks(self):
        """Compute the masks for the corners of the square

        E.g. 2x2, 3x3, 4x4, 5x5

        NOTE: we can make this more time efficient by storing
        it as a dictionary of move(s) -> masks
        """
        masks = []
        for size in range(2, min(self.rows, self.cols) + 1):
            top_left = 0

            for row_offset in range(self.rows - size + 1):
                for col_offset in range(self.cols - size + 1):
                    mask = 0
                    top_left = row_offset * self.cols + col_offset

                    top_right = top_left + size - 1
                    bottom_left = top_left + (size - 1) * self.cols
                    bottom_right = bottom_left + size - 1

                    mask |= 1 << top_left
                    mask |= 1 << top_right
                    mask |= 1 << bottom_left
                    mask |= 1 << bottom_right

                    masks.append(mask)

        return masks


class Square:
    """

    Board is represented using a bitboard.
    Each board is a "rows x cols" -bit integer

    """

    def __init__(self, config: SquareConfig, debug=False):
        self.config = config
        self.boards = [0 for _ in range(self.config.players)]
        self.corner_masks = self.config.compute_square_corner_masks()
        print(f"Number of corner masks: {len(self.corner_masks)}")

        if debug:
            print("Corner masks:")
            for mask in self.corner_masks:
                self.boards[0] = mask
                print(self.show())
                input()
                self.reset()

    def reset(self):
        self.boards = [0 for _ in range(self.config.players)]

    def move_to_bitboard(self, move: tuple) -> int:
        """Convert a move to a bitboard representation

        NOTE: we do: row * config.cols + col to get the index of the bit.
        (0, 0) -> 0
        (0, 1) -> 1
        (1, 0) -> config.cols (5)
        """
        row, col = move
        assert 0 <= row < self.config.rows
        assert 0 <= col < self.config.cols
        return 1 << (row * self.config.cols + col)

    def show(self):
        out = ""

        for row, col in product(range(self.config.rows), range(self.config.cols)):
            if col == 0:
                out += f"\n{row}|"

            position = self.move_to_bitboard((row, col))
            for player in range(self.config.players):
                if self.boards[player] & position:
                    out += str(player)
                    break
            else:
                out += "."

        return out

    def is_valid_move(self, move: tuple, move_bitboard: int | None = None):
        """
        1. Ensure the move is within the bounds of the board
        2. Check if no other player has occupied the position
        """
        row, col = move

        if move_bitboard is None:
            move_bitboard = self.move_to_bitboard(move)

        return (
            0 <= row < self.config.rows
            and 0 <= col < self.config.cols
            and all(
                self.boards[player] & move_bitboard == 0
                for player in range(self.config.players)
            )
        )

    def play_move(self, move: tuple, player: int):
        move_bitboard = self.move_to_bitboard(move)
        assert self.is_valid_move(move, move_bitboard)
        self.boards[player] |= move_bitboard

    def check_corners_make_square(self, player: int):
        """Check if player has made any sized square by occupying the corners

        E.g. 2x2, 3x3, 4x4, 5x5
        """
        board = self.boards[player]
        for mask in self.corner_masks:
            # Check if *all* corners are occupied
            if board & mask == mask:
                return True
        return False


if __name__ == "__main__":
    s = Square(SquareConfig())
    print(s.show())

    move = (0, 1)  # row, col
    print(f"\nMove: {move}")
    s.play_move(move, player=0)
    print(s.show())

    assert s.is_valid_move(move) is False

    move = (2, 1)
    print(f"\nMove: {move}")
    s.play_move(move, player=1)
    print(s.show())
