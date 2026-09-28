"""Minimax search (negamax with alpha-beta pruning) for Complete The Square.

See docs/ai-search.md for a walkthrough of how this works.
"""

from bitboard import BitboardGame, BitboardState, GameConfig

# --- Score scale --------------------------------------------------------------
#
# Every score is "from the point of view of the player who is about to move".
#
#   forced win  :  +WIN_SCORE (+ a small bonus for winning sooner)
#   forced loss :  -WIN_SCORE (- the same bonus, so losing later is preferred)
#   unknown     :  a heuristic guess strictly inside (-HEURISTIC_CAP, +HEURISTIC_CAP)
#
# Because HEURISTIC_CAP < WIN_SCORE, no amount of "nice position" can ever
# outrank a real win or loss found by the search.
WIN_SCORE = 1.0
WIN_DEPTH_BONUS = 0.01  # win with N plies of depth left scores WIN_SCORE + N * bonus
HEURISTIC_CAP = 0.5

# Weights for the three heuristic terms.  They sum to HEURISTIC_CAP, and each
# term is normalised to [-1, 1], so the total is always within the cap.
W_MATERIAL = 0.25  # piece count difference
W_POSITION = 0.10  # corners and edges are hard/impossible to capture
W_THREATS = 0.15  # squares where a player owns 3 corners and the 4th is empty
MAX_THREATS = 3  # threat difference is clamped to +-MAX_THREATS before scaling


class Evaluator:
    """Precomputes the masks the heuristic needs for a given board size."""

    def __init__(self, game: BitboardGame):
        self.game = game
        rows, cols = game.config.rows, game.config.cols

        corner_mask = edge_mask = 0
        for bit_index, (r, c) in enumerate(game.index_to_square):
            on_row_edge = r in (0, rows - 1)
            on_col_edge = c in (0, cols - 1)
            if on_row_edge and on_col_edge:
                corner_mask |= 1 << bit_index
            elif on_row_edge or on_col_edge:
                edge_mask |= 1 << bit_index
        self.corner_mask = corner_mask
        self.edge_mask = edge_mask
        # A corner can never be captured (no line through it has both sides on
        # the board); an edge square can only be captured along the edge.
        # Weight them 2:1 and normalise so a full board of corners+edges = 1.0.
        self.position_norm = 2 * corner_mask.bit_count() + edge_mask.bit_count()
        self.material_norm = game.num_squares

        # Move ordering for the search: corners first, then squares by distance
        # from the nearest corner.  This matches what the evaluation values
        # (corners and edges are the safest squares), so strong moves are
        # tried early and alpha-beta prunes well.  It is also the tie-break
        # between equally scored moves, which is why the AI opens in a corner.
        def corner_distance(i: int) -> int:
            r, c = game.index_to_square[i]
            return min(r, rows - 1 - r) + min(c, cols - 1 - c)

        order = sorted(range(game.num_squares), key=corner_distance)
        self.move_order = [1 << i for i in order]

    def threat_difference(self, mine: int, theirs: int) -> int:
        """(my threats) - (their threats).

        A threat is a square whose 4 corners hold 3 of one player's pieces and
        nothing of the other player's, so one more move completes it.
        """
        diff = 0
        for mask in self.game.all_corner_masks:
            m, t = mine & mask, theirs & mask
            if not t:
                if m.bit_count() == 3:
                    diff += 1
            elif not m and t.bit_count() == 3:
                diff -= 1
        return diff

    def evaluate(self, state: BitboardState) -> float:
        """Heuristic score of ``state`` for the player about to move."""
        me = state.current_player
        mine = state.boards[me]
        theirs = 0
        for p, board in enumerate(state.boards):
            if p != me:
                theirs |= board

        material = (mine.bit_count() - theirs.bit_count()) / self.material_norm

        def positional(board: int) -> int:
            return 2 * (board & self.corner_mask).bit_count() + (board & self.edge_mask).bit_count()

        position = (positional(mine) - positional(theirs)) / self.position_norm

        threat_diff = self.threat_difference(mine, theirs)
        threat_diff = max(-MAX_THREATS, min(MAX_THREATS, threat_diff))
        threats = threat_diff / MAX_THREATS

        return W_MATERIAL * material + W_POSITION * position + W_THREATS * threats


def ordered_moves(evaluator: Evaluator, empty: int, killer: int):
    """Yield legal move bits: the killer move first, then centre-out."""
    if killer & empty:
        yield killer
    for bit in evaluator.move_order:
        if bit & empty and bit != killer:
            yield bit


def negamax(
    game: BitboardGame,
    evaluator: Evaluator,
    state: BitboardState,
    depth: int,
    alpha: float,
    beta: float,
    killers: list[int],
) -> float:
    """Score ``state`` for the player to move, searching ``depth`` plies ahead.

    ``alpha`` is the best score the player to move is already guaranteed
    elsewhere in the tree; ``beta`` is the best the opponent is guaranteed.
    Once we find a move scoring >= beta the opponent would never let us reach
    this position, so the remaining moves are skipped (the "cut-off").

    ``killers[depth]`` remembers the move that most recently caused a cut-off
    at this depth.  A move that refuted one sibling position usually refutes
    the next one too, so trying it first makes cut-offs happen much sooner.
    """
    if depth == 0:
        return evaluator.evaluate(state)

    empty = game.empty_mask(state)
    if not empty:
        return 0.0  # board full: draw

    best = float("-inf")
    for bit in ordered_moves(evaluator, empty, killers[depth]):
        new_state, won = game.play_move_bit(bit, state)
        if won:
            # Nothing can beat an immediate win, so stop looking.
            return WIN_SCORE + WIN_DEPTH_BONUS * depth

        # The opponent's score for the child is the negative of ours, and
        # their (alpha, beta) window is our window flipped and negated.
        score = -negamax(game, evaluator, new_state, depth - 1, -beta, -alpha, killers)

        if score > best:
            best = score
        if best > alpha:
            alpha = best
        if alpha >= beta:
            killers[depth] = bit
            break  # cut-off: the opponent has a better option earlier in the tree
    return best


def find_best_move(
    game: BitboardGame,
    state: BitboardState,
    depth: int,
    evaluator: Evaluator | None = None,
) -> tuple[int, int]:
    """Return the (row, col) the player to move should play."""
    evaluator = evaluator or Evaluator(game)

    empty = game.empty_mask(state)
    best_bit = None
    best_score = float("-inf")
    alpha, beta = float("-inf"), float("inf")
    killers = [0] * (depth + 1)

    for bit in ordered_moves(evaluator, empty, 0):
        new_state, won = game.play_move_bit(bit, state)
        if won:
            best_bit = bit
            break
        score = -negamax(game, evaluator, new_state, depth - 1, -beta, -alpha, killers)
        if score > best_score:
            best_score, best_bit = score, bit
        alpha = max(alpha, best_score)

    if best_bit is None:
        return (0, 0)  # no legal moves
    return game.index_to_square[best_bit.bit_length() - 1]


def play_minimax_game(game: BitboardGame, depth: int = 5):
    """Play a game in the terminal: player 0 is human, player 1 is the AI."""
    state = game.new_game_state()
    evaluator = Evaluator(game)

    while True:
        print(game.show(state))
        if state.current_player == 0:
            move = input(f"Player {state.current_player}, enter your move (row,col): ")
            if move.lower() == "exit":
                break
            try:
                row, col = map(int, move.split(","))
                state, winner = game.play_move((row, col), state)
            except Exception as e:
                print(f"Invalid move: {e}")
                continue
        else:
            move = find_best_move(game, state, depth, evaluator)
            state, winner = game.play_move(move, state)
            print(f"AI played move: {move}")

        if winner:
            print(game.show(state))
            print(f"Player {state.current_player} wins!")
            break


if __name__ == "__main__":
    play_minimax_game(BitboardGame(GameConfig(players=2, rows=5, cols=5)))
