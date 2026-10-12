// The rules, from the same ai.wasm the page uses: replay a game and describe
// the position, for the board view and as text for Claude.

import wasm from './ai.wasm';

const engine = new WebAssembly.Instance(wasm, {}).exports;

export const SIZE = 5;
const COLUMNS = 'ABCDE';
export const PLAYERS = ['green', 'red'];

export const other = (colour) => (colour === 'green' ? 'red' : 'green');

export function squareName(index) {
  return COLUMNS[index % SIZE] + (Math.floor(index / SIZE) + 1);
}

// "c3" -> its index; throws on anything that is not a square.
export function parseSquare(text) {
  const match = /^\s*([A-Ea-e])([1-5])\s*$/.exec(String(text));
  if (!match) throw new Error(`"${text}" is not a square: use a column A-E and a row 1-5, like C3.`);
  return (Number(match[2]) - 1) * SIZE + COLUMNS.indexOf(match[1].toUpperCase());
}

function squaresOf(mask) {
  const names = [];
  for (let i = 0; i < SIZE * SIZE; i++) if (mask & (1 << i)) names.push(squareName(i));
  return names;
}

function moveText(names) {
  const pairs = [];
  for (let i = 0; i < names.length; i += 2) pairs.push(`${i / 2 + 1}. ${names.slice(i, i + 2).join(' ')}`);
  return pairs.join('  ');
}

function asciiBoard(green, red) {
  const rows = [`   ${COLUMNS.split('').join(' ')}`];
  for (let row = 0; row < SIZE; row++) {
    const cells = [];
    for (let col = 0; col < SIZE; col++) {
      const bit = 1 << (row * SIZE + col);
      cells.push(green & bit ? 'G' : red & bit ? 'R' : '.');
    }
    rows.push(`${row + 1}  ${cells.join(' ')}`);
  }
  return rows.join('\n');
}

// The position after `squares` (indices, in order), or throws if a move is illegal.
export function position(squares) {
  engine.reset();
  for (const [i, square] of squares.entries()) {
    const over = engine.winner() >= 0 || engine.draw();
    if (!engine.play(Math.floor(square / SIZE), square % SIZE)) {
      throw new Error(`${squareName(square)} is illegal: ${over ? 'the game is over' : 'that square is taken'}.`);
    }
  }
  const ply = squares.length;
  const green = engine.board(0);
  const red = engine.board(1);
  const mover = ply % 2 === 1 ? 0 : 1; // who played the last move
  const captured = ply ? engine.board_at(ply - 1, 1 - mover) & ~engine.board(1 - mover) : 0;
  const winner = engine.winner();
  const draw = engine.draw() === 1;
  const moves = squares.map(squareName);
  return {
    moves,
    green: squaresOf(green),
    red: squaresOf(red),
    last: ply ? moves[ply - 1] : null,
    captured: squaresOf(captured),
    toMove: winner >= 0 || draw ? null : PLAYERS[engine.current_player()],
    winner: winner >= 0 ? PLAYERS[winner] : null,
    winningSquare: squaresOf(engine.winning_mask()),
    draw,
    board: asciiBoard(green, red),
    moveText: moveText(moves),
  };
}
