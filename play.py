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
        # Need to check if out of range of board

    def _check_for_win(self, newPos, player):
        pass

    def _check_for_piece_removal(self, newPos, player):
        pass
    

    def make_move(self, player, newPos):
        if player==1:
            current_player = self.player1
        else:
            current_player = self.player2       # Objects are passed by reference    
        if self._valid_move(newPos):
            current_player.update_pieces
            self._check_for_win(newPos, player)
            self._check_for_piece_removal( newPos, player)
        else:
            print("This was an invalid move")


def translate(move):
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

    return int(newFirst+newSecond)


print("Player 1 input your name:")
curr = input()
player1 = Player(curr, 'W')

print("Player 2 input your name:")
curr = input()
player2 = Player(curr, 'B')

game = Game(player1, player2)

while True:

    print("Your move Player " + player1.name)
    move = input()
    transMove = translate(move)
    game.make_move(1, transMove)

    print("Your move Player " + player2.name)
    move = input()
    transMove = translate(move)
    game.make_move(2, transMove)
