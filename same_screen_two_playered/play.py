import pickle

class Player:
    """A white or black player"""

    def __init__(self, name, colour):
        self.name = name
        self.colour = colour
        self.pieces = []    # List of positions of all pieces placed on board

    def update_pieces(self, newPos):
        self.pieces.append(newPos)

class Game:
    """The current game in play"""

    def __init__(self, player1, player2):
        self.player1 = player1
        self.player2 = player2

    def _valid_move(self, newPos):
        valid = True
        if newPos in self.player1.pieces:
            valid = False
        if newPos in self.player2.pieces:
            valid = False
        if int(newPos[0]) < 1 or int(newPos[0]) > 5:
            valid = False
        if int(newPos[1]) < 1 or int(newPos[1]) > 5:
            valid = False
        return valid

    def _get_diagonals(self, pos):
        diags = []
        # Get leading diagonal positions first
        outOfRange = False
        curr = pos
        while not(outOfRange):
            newRow = int(curr[0])+1
            newCol = int(curr[1])+1
            if newRow > 5 or newCol > 5:
                outOfRange = True
            else:
                curr = str(newRow)+str(newCol)
                diags.append(curr)
        
        outOfRange = False
        curr = pos
        while not(outOfRange):
            newRow = int(curr[0])+1
            newCol = int(curr[1])-1
            if newRow > 5 or newCol < 1:
                outOfRange = True
            else:
                curr = str(newRow)+str(newCol)
                diags.append(curr)

        outOfRange = False
        curr = pos
        while not(outOfRange):
            newRow = int(curr[0])-1
            newCol = int(curr[1])+1
            if newRow < 1 or newCol > 5:
                outOfRange = True
            else:
                curr = str(newRow)+str(newCol)
                diags.append(curr)

        outOfRange = False
        curr = pos
        while not(outOfRange):
            newRow = int(curr[0])-1
            newCol = int(curr[1])-1
            if newRow < 1 or newCol < 1:
                outOfRange = True
            else:
                curr = str(newRow)+str(newCol)
                diags.append(curr)

        return diags

    def _get_neighbours(self, pos):
        neighbs = []

        curr = pos
        newRow = int(curr[0])+1
        newCol = int(curr[1])+1
        if newRow > 5 or newCol > 5:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])+1
        newCol = int(curr[1])+0
        if newRow > 5:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])+0
        newCol = int(curr[1])+1
        if newCol > 5:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])+1
        newCol = int(curr[1])-1
        if newRow > 5 or newCol < 1:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])-1
        newCol = int(curr[1])+1
        if newRow < 1 or newCol > 5:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])-1
        newCol = int(curr[1])-1
        if newRow < 5 or newCol < 5:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])-1
        newCol = int(curr[1])+0
        if newRow < 1:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        curr = pos
        newRow = int(curr[0])+0
        newCol = int(curr[1])-1
        if newCol < 1:
            # Out of range
            pass
        else:
            curr = str(newRow)+str(newCol)
            neighbs.append(curr)

        return neighbs

    def _check_for_win(self, newPos, player):
        # Check the newest position forms a square with any other three older positions
        hasWon = False
        """Method:
        - Get list of all positions on diagonal of current position 
        - Hence, get coordinates of all possible (any size) squares that can be formed using current square
        - For each such square, check the remaining three positions are in the history of all positions in which player has counters
        """
        diags = self._get_diagonals(newPos)

        for diag in diags:
            if diag in player.pieces:
                square1 = newPos[0] + diag[1]
                square2 = diag[0] + newPos[1]
                if square1 in player.pieces and square2 in player.pieces:
                    hasWon = True
                    print("Player " + player.name + " has won!")
        
        return hasWon

    def _check_for_piece_removal(self, newPos, player):
        if player==1:
            this_player = self.player1
            other_player = self.player2
        else:
            this_player = self.player2
            other_player = self.player1

        neighbours = self._get_neighbours(newPos)

        to_delete = []
        for neighb in neighbours:
            if neighb in other_player.pieces:
                # Check 2 away in this line
                diffx = int(neighb[0]) - int(newPos[0])
                diffy = int(neighb[1]) - int(newPos[1])
                newRow = int(neighb[0]) + diffx
                newCol = int(neighb[1]) + diffy
                if (newRow < 1 or newRow > 5) or (newCol < 1 or newCol > 5):
                    # Out of range
                    pass
                else:
                    neighbNeighb = str(newRow)+str(newCol)
                    if neighbNeighb in this_player.pieces:
                        to_delete.append(neighb)
                    if neighbNeighb in other_player.pieces:
                        # Check 3 away in this line
                        diffx = int(neighbNeighb[0]) - int(neighb[0])
                        diffy = int(neighbNeighb[1]) - int(neighb[1])
                        newRow = int(neighbNeighb[0]) + diffx
                        newCol = int(neighbNeighb[1]) + diffy
                        if (newRow < 1 or newRow > 5) or (newCol < 1 or newCol > 5):
                            # Out of range
                            pass
                        else:
                            neighbNeighbNeighb = str(newRow)+str(newCol)
                            if neighbNeighbNeighb in this_player.pieces:
                                to_delete.append(neighb)
                                to_delete.append(neighbNeighb)
                            if neighbNeighbNeighb in other_player.pieces:
                                # Check 4 away in this line
                                diffx = int(neighbNeighbNeighb[0]) - int(neighbNeighb[0])
                                diffy = int(neighbNeighbNeighb[1]) - int(neighbNeighb[1])
                                newRow = int(neighbNeighbNeighb[0]) + diffx
                                newCol = int(neighbNeighbNeighb[1]) + diffy
                                if (newRow < 1 or newRow > 5) or (newCol < 1 or newCol > 5):
                                    # Out of range
                                    pass
                                else:
                                    neighbNeighbNeighbNeighb = str(newRow)+str(newCol)
                                    if neighbNeighbNeighbNeighb in this_player.pieces:
                                        to_delete.append(neighb)
                                        to_delete.append(neighbNeighb)
                                        to_delete.append(neighbNeighbNeighb)

        # Remove deleted pieces
        for piece in to_delete:
            other_player.pieces.remove(piece)
            print("Piece deleted!!")





    def make_move(self, player, newPos):
        if player==1:
            current_player = self.player1
        else:
            current_player = self.player2       # Objects are passed by reference    
        if self._valid_move(newPos):
            current_player.update_pieces(newPos)
            gameFinished = self._check_for_win(newPos, current_player)
            self._check_for_piece_removal( newPos, player)
            return gameFinished
        else:
            print("This was an invalid move")
            return None

def initialise_game(name1, name2, curr_game_name):
    player1 = Player(name1, 'W')
    player2 = Player(name2, 'B')
    game = Game(player1, player2)
    
    object_list = [player1, player2, game]
    # Create file and write objects to it
    file_name = curr_game_name+'.pkl'
    with open(file_name, 'w') as f:
        pickle.dump(object_list, f)

def receive_move(as_json):
    # Retrieve objects from file of curr_game_name
    # Determine whether the newest move is made by player 1 or player 2
    # 'player' is a boolean variable

    as_dict = json.loads(as_json)
    move = as_dict['move']
    curr_game_name = as_dict['curr_game_name']

    # Retrieve objects from file of curr_game_name
    # Determine whether the newest move is made by player 1 or player 2

    transMove = translate(move)
    gameFinished = game.make_move(player, transMove)

    if gameFinished:
        print("Game Over")
