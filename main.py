import os

from flask import Flask, jsonify, request
from flask_cors import CORS

from bitboard import BitboardGame, BitboardState, GameConfig
from minimax import find_best_move

app = Flask(__name__)
CORS(
    app,
    origins=[
        "https://guppy16.github.io",
        "http://localhost:3000",
        "http://0.0.0.0:3000",
    ],
    methods=["GET", "POST", "OPTIONS"],
    allow_headers=["Content-Type"],
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


@app.route("/", methods=["GET"])
def index():
    return jsonify({"message": "Welcome to the Bitboard Game API!"})


@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "healthy"}), 200


# For production WSGI servers
application = app

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 8080))
    debug = os.environ.get("FLASK_ENV") == "development"
    print(f"Starting Flask app on 0.0.0.0:{port}")
    app.run(host="0.0.0.0", port=port, debug=debug)
