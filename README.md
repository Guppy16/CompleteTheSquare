# Complete the Square Game

View the [Complete the Square game online](https://Guppy16.github.io/CompleteTheSquare/).

Adapted from: https://github.com/VatsalRaina/CompleteTheSquare


### Development

To develop the js game locally, you can use the following commands:

```bash
python3 -m http.server 3000
```

Then open your browser and navigate to `http://localhost:3000/`.

### AI backend

`main.py` is a Flask app that returns the AI's move (`bitboard.py` is the rules engine,
`minimax.py` the search). How it works, including alpha-beta pruning and the bitboard
tricks, is explained in [docs/ai-search.md](docs/ai-search.md).

```bash
python3 test_ai.py   # regression tests
python3 minimax.py   # play against the AI in the terminal
```
