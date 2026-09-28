// Complete the Square: page logic.
//
// All game state and rules live in the WebAssembly module (built from wasm/).
// This file only forwards clicks to it and redraws the board from its state.

const BOARD_SIZE = 5;
const AI_DEPTH = 7;                    // plies; ~100 ms per move on a laptop
const WASM_URL = 'square-game/ai.wasm';
const COLOURS = ['W', 'B'];            // player 0 is green (W), player 1 is red (B)

let ai = null;                         // the module's exports, once loaded
let playAgainstAI = false;             // default: two humans, one screen

const boardEl = document.getElementById('board');
const statusEl = document.getElementById('status');
const playAgainButton = document.getElementById('play-again');
const aiToggle = document.getElementById('ai-toggle');
const leftOption = document.querySelector('.switch-option.left');
const rightOption = document.querySelector('.switch-option.right');
const cells = [];                      // index = row * BOARD_SIZE + col, same as the bitboard

document.documentElement.style.setProperty('--board-size', BOARD_SIZE);

function getColorName(player) {
  return player === 0 ? 'Green' : 'Red';
}

function getHexColor(player) {
  return player === 0 ? '#4CAF50' : '#F44336';
}

function createColoredStatus(player, isWinner = false) {
  const colorName = getColorName(player);
  const hexColor = getHexColor(player);
  const badge = `<span style="background-color: ${hexColor}50; padding: 2px 8px; border-radius: 4px; text-shadow: 0 0 3px ${hexColor}80;">`;
  return isWinner ? `${badge}${colorName} wins!</span>` : `${badge}${colorName}</span> to move`;
}

function isAITurn() {
  return playAgainstAI && ai.winner() < 0 && ai.current_player() === 1;
}

// Redraw everything from the module's state.
function render() {
  const boards = [ai.board(0), ai.board(1)];
  const winner = ai.winner();
  const winMask = ai.winning_mask();

  cells.forEach((cell, i) => {
    const bit = 1 << i;
    cell.className = 'cell';
    boards.forEach((board, player) => {
      if (board & bit) cell.classList.add(`player-${COLOURS[player]}`, 'disabled');
    });
    if (winner >= 0) cell.classList.add('disabled');
    if (winMask & bit) cell.classList.add('winner');
  });

  statusEl.innerHTML = winner >= 0
    ? createColoredStatus(winner, true)
    : createColoredStatus(ai.current_player());
  playAgainButton.hidden = winner < 0;
  boardEl.classList.remove('thinking');
}

function aiMove() {
  ai.ai_play(AI_DEPTH);
  render();
}

function onCellClick(row, col) {
  if (!ai || isAITurn()) return;       // not loaded yet, or waiting for the AI
  if (!ai.play(row, col)) return;      // occupied, or game over
  render();

  if (isAITurn()) {
    statusEl.textContent = 'AI is thinking…';
    boardEl.classList.add('thinking');
    setTimeout(aiMove, 20);            // let the browser paint the human's move first
  }
}

function newGame() {
  ai.reset();
  render();
}

function buildBoard() {
  boardEl.innerHTML = '';
  cells.length = 0;
  for (let r = 0; r < BOARD_SIZE; r++) {
    for (let c = 0; c < BOARD_SIZE; c++) {
      const cell = document.createElement('div');
      cell.classList.add('cell');
      cell.addEventListener('click', () => onCellClick(r, c));
      boardEl.appendChild(cell);
      cells.push(cell);
    }
  }
}

aiToggle.addEventListener('change', () => {
  playAgainstAI = aiToggle.checked;
  leftOption.classList.toggle('active', !playAgainstAI);
  rightOption.classList.toggle('active', playAgainstAI);
  if (ai) newGame();
});

playAgainButton.addEventListener('click', () => {
  if (ai) newGame();
});

async function loadAI() {
  const bytes = await (await fetch(WASM_URL)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  ai = instance.exports;
  newGame();
}

buildBoard();
statusEl.textContent = 'Loading…';
loadAI().catch(err => {
  console.error(err);
  statusEl.textContent = 'Could not load the game module. Try reloading the page.';
});
