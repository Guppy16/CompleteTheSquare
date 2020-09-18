import pickle
import json
import os

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
        self.all_moves = []

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
            self.all_moves.append(newPos)
            gameFinished = self._check_for_win(newPos, current_player)
            self._check_for_piece_removal( newPos, player)
            return gameFinished
        else:
            print("This was an invalid move")
            return None

def translate_coordinate_to_num(move):
    """Translate from a letter-number system to an integer convention"""
    firstChar = move[0]
    secondChar = move[1]

    newSecond = secondChar
    if firstChar=='a':
        newFirst = '1'
    elif firstChar=='b':
        newFirst = '2'
    elif firstChar=='c':
        newFirst = '3'
    elif firstChar=='d':
        newFirst = '4'
    elif firstChar=='e':
        newFirst = '5'

    return newFirst+newSecond

def translate_num_to_coordinate(move):
    """DeTranslate from a integer convention system to letter-number"""
    firstChar = move[0]
    secondChar = move[1]

    newSecond = secondChar
    if firstChar=='1':
        newFirst = 'a'
    elif firstChar=='2':
        newFirst = 'b'
    elif firstChar=='3':
        newFirst = 'c'
    elif firstChar=='4':
        newFirst = 'd'
    elif firstChar=='5':
        newFirst = 'e'

    return newFirst+newSecond

def initialise_game(name1, name2, curr_game_name):
    player1 = Player(name1, 'W')
    player2 = Player(name2, 'B')
    game = Game(player1, player2)

    # Create file and write game object to it
    file_name = curr_game_name+'.pkl'
    with open(file_name, 'wb') as f:
        pickle.dump(game, f)

def receive_move(as_json):

    #as_dict = json.loads(as_json)
    move = as_json['move']
    curr_game_name = as_json['curr_game_name']

    # Retrieve objects from file of curr_game_name
    file_name = curr_game_name+'.pkl'
    with open(file_name, 'rb') as f:
        game = pickle.load(f)

    # Determine whether the newest move is made by player 1 or player 2
    move_num = len(game.all_moves)
    if move_num%2 == 0:
        #even number of moves
        player_num = 1
    else:
        player_num = 2

    # Translate move to style used for game class
    transMove = translate_coordinate_to_num(move)

    # Update the game object
    gameFinished = game.make_move(player_num, transMove)

    
    game_over = 0
    if gameFinished:
        print("Game Over")
        # Save all game moves to a central file

        game_over = 1

        # Delete the temporary game pickle file
        os.remove(file_name)
    else:
        # Write updated game object to temporary pickle file
        with open(file_name, 'wb') as f:
            pickle.dump(game, f)

    player1_pos = game.player1.pieces
    player2_pos = game.player2.pieces

    # Translate back to letter-number system
    player1_pos = [translate_num_to_coordinate(pos) for pos in player1_pos]
    player2_pos = [translate_num_to_coordinate(pos) for pos in player2_pos]

    message = {"p1": player1_pos, "p2": player2_pos, "game_over": game_over}
    return message

# def get_positions(req):
#     curr_game_name = req['curr_game_name']
#     # Retrieve objects from file of curr_game_name
#     file_name = curr_game_name+'.pkl'
#     with open(file_name, 'rb') as f:
#         game = pickle.load(f)

#     player1_pos = game.player1.pieces
#     player2_pos = game.player2.pieces

#     # Translate back to letter-number system
#     player1_pos = [translate_num_to_coordinate(pos) for pos in player1_pos]
#     player2_pos = [translate_num_to_coordinate(pos) for pos in player2_pos]

#     message = {"p1": player1_pos, "p2": player2_pos}
#     return message