import torch


class SquareGame:
    def __init__(self, board_size=5, batch_size=1024, device="cuda"):
        self.board_size = board_size
        self.batch_size = batch_size
        self.device = device

        # Create tensors for game state (all on GPU)
        self.reset()

        # Pre-compute all possible squares (static tensor)
        self.possible_squares = self._precompute_squares().to(device)

        # Pre-compute direction kernels for captures
        self.direction_offsets = self._precompute_directions().to(device)

    def reset(self):
        # Initialize board states: [batch, 2, height, width]
        # Channel 0: Player 1 pieces, Channel 1: Player 2 pieces
        self.boards = torch.zeros(
            self.batch_size,
            2,
            self.board_size,
            self.board_size,
            dtype=torch.bool,
            device=self.device,
        )

        # Current player: 0 = player 1, 1 = player 2
        self.current_players = torch.zeros(
            self.batch_size, dtype=torch.int64, device=self.device
        )

        # Game finished indicators
        self.done = torch.zeros(self.batch_size, dtype=torch.bool, device=self.device)

        # Winners: -1 = no winner, 0 = player 1, 1 = player 2
        self.winners = torch.full(
            (self.batch_size,), -1, dtype=torch.int8, device=self.device
        )

    def _precompute_squares(self):
        # Create a list of all possible squares on the board
        # Each square is represented by the coordinates of its 4 corners
        squares = []

        for size in range(1, self.board_size):
            for r in range(self.board_size - size + 1):
                for c in range(self.board_size - size + 1):
                    # Square with (r,c) as top-left
                    squares.append(
                        torch.tensor(
                            [[r, c], [r, c + size], [r + size, c], [r + size, c + size]]
                        )
                    )

                    # Add other square configurations...

        return torch.stack(squares)

    def _precompute_directions(self):
        # Create kernels for the 8 possible directions for captures
        return torch.tensor(
            [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]],
            device=self.device,
        )

    def make_moves(self, moves):
        """Process moves for all games in the batch"""
        # moves: [batch_size, 2] with each move as [row, col]

        # 1. Update boards with new pieces
        batch_indices = torch.arange(self.batch_size, device=self.device)
        player_indices = self.current_players

        # Place pieces on the board
        self.boards[batch_indices, player_indices, moves[:, 0], moves[:, 1]] = True

        # 2. Check for winning squares
        self._check_for_winners(moves)

        # 3. Process captures
        self._process_captures(moves)

        # 4. Switch players
        self.current_players = 1 - self.current_players

        return self._get_observations()

    def _check_for_winners(self, moves):
        """Check if any games in the batch have been won"""
        # Implementation using tensor operations to check all possible squares
        # This would use the pre-computed squares tensor
        pass

    def _process_captures(self, moves):
        """Process piece captures for all games in batch"""
        # Implementation using convolution or indexing operations
        pass

    def _get_observations(self):
        """Return current game state as observation tensors"""
        # Format suitable for neural network input
        obs = torch.cat(
            [
                self.boards,
                self.current_players.view(self.batch_size, 1, 1, 1).expand(
                    -1, 1, self.board_size, self.board_size
                ),
            ],
            dim=1,
        )

        return {"observation": obs, "done": self.done, "winner": self.winners}
