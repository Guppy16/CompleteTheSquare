# Trap, Capture, Complete The Square

A two-player abstract strategy game in Rust. Play against the engine (alpha-beta search)
in your browser as WebAssembly. Can you solve the opening?

**[Play it](https://Guppy16.github.io/CompleteTheSquare/)** · [Rules](https://guppy16.github.io/CompleteTheSquare/docs/00-the-game.html) · [How the engine works](https://guppy16.github.io/CompleteTheSquare/docs/)

<p align="center">
  <img src="docs/img/how-to-play.svg" alt="A short game with captures on both sides, ending with Green completing the square A1 to D4" width="300">
</p>
<p align="center"><sub>Complete the Square Gameplay</sub></p>

## Analysis mode

Play against the engine, or have it show its top lines for every position, as on lichess.
Each candidate square holds a mini board of where its line ends.

<p align="center">
  <img src="docs/img/analysis.svg" alt="The Analysis tab: the engine's top lines above the board, a marker on each candidate square, and the eval bar, over three moves" width="320">
</p>

## Credits

- Game adapted from [VatsalRaina/CompleteTheSquare](https://github.com/VatsalRaina/CompleteTheSquare).
- Motion modelled on [Kinetics](https://github.com/ckissi/kinetics), a gallery of
  spring-physics interface animations.

## Development

- Layout, commands and tools: [CLAUDE.md](CLAUDE.md).
- Open questions and the to-do list: [Next steps](https://guppy16.github.io/CompleteTheSquare/docs/18-next-steps.html).
