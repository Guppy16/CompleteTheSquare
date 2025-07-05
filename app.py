from flask import Flask, jsonify, request
from flask_cors import CORS

from bitboard import BitboardGame, BitboardState, GameConfig
from minimax import find_best_move

app = Flask(__name__)
CORS(
    app,
    origins=["https://guppy16.github.io/CompleteTheSquare/", "http://localhost:3000"],
)

# Set up your game config
config = GameConfig(players=2, rows=5, cols=5)
game = BitboardGame(config)


@app.route("/ai-move", methods=["POST"])
def ai_move():
    data = request.json
    print(data)
    state = BitboardState(
        boards=tuple(data["boards"]),
        current_player=data["current_player"],
    )
    best_move = find_best_move(game, state, depth=3)
    return jsonify({"move": best_move})


if __name__ == "__main__":
    app.run(port=5000, debug=True)
