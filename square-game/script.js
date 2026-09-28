// Complete the Square: page logic.
//
// All game state and rules live in the WebAssembly module (built from wasm/).
// This file only forwards clicks to it and redraws the board from its state.

const BOARD_SIZE = 5;
// The AI searches deeper and deeper until it has visited AI_NODE_BUDGET
// positions (about 100 ms on a laptop, well under a second on a phone) or
// reached AI_MAX_DEPTH plies.
const AI_MAX_DEPTH = 12;
const AI_NODE_BUDGET = 200000;
const WASM_URL = 'square-game/ai.wasm';
const COLOURS = ['W', 'B'];            // player 0 is green (W), player 1 is red (B)
const COLUMN_LABELS = 'ABCDEFGHIJ';     // columns are lettered, rows numbered from the top

let ai = null;                         // the module's exports, once loaded
let playAgainstAI = false;             // default: two humans, one screen

const boardEl = document.getElementById('board');
const statusEl = document.getElementById('status');
const playAgainButton = document.getElementById('play-again');
const undoButton = document.getElementById('undo');
const redoButton = document.getElementById('redo');
const copyButton = document.getElementById('copy-moves');
const moveListEl = document.getElementById('move-list');
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

function gameOver() {
  return ai.winner() >= 0 || ai.draw() === 1;
}

function isAITurn() {
  return playAgainstAI && !gameOver() && ai.current_player() === 1;
}

// Square index -> "C3": columns lettered, rows numbered from 1 at the top.
function squareName(index) {
  return COLUMN_LABELS[index % BOARD_SIZE] + (Math.floor(index / BOARD_SIZE) + 1);
}

// The moves so far as numbered pairs: ["1. C3 A1", "2. B2 D4", "3. E5"].
function movePairs() {
  const pairs = [];
  const count = ai.move_count();
  for (let i = 0; i < count; i += 2) {
    let pair = `${i / 2 + 1}. ${squareName(ai.move_at(i))}`;
    if (i + 1 < count) pair += ` ${squareName(ai.move_at(i + 1))}`;
    pairs.push(pair);
  }
  return pairs;
}

function moveText() {
  return movePairs().join('  ');
}

function renderMoves() {
  moveListEl.innerHTML = '';
  const pairs = movePairs();
  if (pairs.length === 0) {
    moveListEl.textContent = 'No moves yet';
    return;
  }
  pairs.forEach(pair => {
    const span = document.createElement('span');
    span.classList.add('pair');
    span.textContent = pair;
    moveListEl.appendChild(span);
  });
}

// Redraw everything from the module's state.
function render() {
  const boards = [ai.board(0), ai.board(1)];
  const winner = ai.winner();
  const winMask = ai.winning_mask();
  const over = gameOver();

  cells.forEach((cell, i) => {
    const bit = 1 << i;
    cell.className = 'cell';
    boards.forEach((board, player) => {
      if (board & bit) cell.classList.add(`player-${COLOURS[player]}`, 'disabled');
    });
    if (over) cell.classList.add('disabled');
    if (winMask & bit) cell.classList.add('winner');
  });

  if (winner >= 0) {
    statusEl.innerHTML = createColoredStatus(winner, true);
  } else if (ai.draw() === 1) {
    statusEl.textContent = 'Draw by threefold repetition';
  } else {
    statusEl.innerHTML = createColoredStatus(ai.current_player());
    if (ai.repetitions() === 2) {
      statusEl.innerHTML += ' <small>(position repeated; a third time is a draw)</small>';
    }
  }
  playAgainButton.hidden = !over;
  // No taking back while the AI is about to move.
  const waiting = isAITurn();
  undoButton.disabled = waiting || ai.move_count() === 0;
  redoButton.disabled = waiting || ai.redo_count() === 0;
  copyButton.disabled = ai.move_count() === 0;
  renderMoves();
  boardEl.classList.remove('thinking');
}

function logMoves() {
  console.log(moveText());
}

// Hand the move to the AI after the browser has painted the current position.
function requestAIMove() {
  statusEl.textContent = 'AI is thinking…';
  boardEl.classList.add('thinking');
  undoButton.disabled = true;
  redoButton.disabled = true;
  setTimeout(aiMove, 20);
}

function aiMove() {
  ai.ai_play(AI_MAX_DEPTH, AI_NODE_BUDGET);
  logMoves();
  render();
}

function onCellClick(row, col) {
  if (!ai || isAITurn()) return;       // not loaded yet, or waiting for the AI
  if (!ai.play(row, col)) return;      // occupied, or game over
  logMoves();
  render();
  if (isAITurn()) requestAIMove();
}

// Against the AI, take back the AI's reply too so it is the human's move again.
function onUndo() {
  if (!ai || isAITurn()) return;
  if (!ai.undo()) return;
  if (playAgainstAI && ai.current_player() === 1) ai.undo();
  render();
  if (isAITurn()) requestAIMove();     // never leave the game waiting on the AI
}

function onRedo() {
  if (!ai || isAITurn()) return;
  if (!ai.redo()) return;
  if (playAgainstAI && ai.current_player() === 1 && ai.redo_count() > 0) ai.redo();
  render();
  if (isAITurn()) requestAIMove();
}

function onCopyMoves() {
  if (!ai) return;
  const text = moveText();
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).catch(() => fallbackCopy(text));
  } else {
    fallbackCopy(text);
  }
}

// Without the clipboard API, select the list so the user can copy it, or show it in a prompt.
function fallbackCopy(text) {
  if (window.getSelection && document.createRange) {
    const range = document.createRange();
    range.selectNodeContents(moveListEl);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    if (document.execCommand && document.execCommand('copy')) return;
  }
  window.prompt('Copy the moves:', text);
}

function newGame() {
  ai.reset();
  render();
}

function addLabel(text) {
  const label = document.createElement('div');
  label.classList.add('label');
  label.textContent = text;
  boardEl.appendChild(label);
}

// The grid has one extra row and column for the coordinate labels.
function buildBoard() {
  boardEl.innerHTML = '';
  cells.length = 0;
  addLabel('');
  for (let c = 0; c < BOARD_SIZE; c++) addLabel(COLUMN_LABELS[c]);
  for (let r = 0; r < BOARD_SIZE; r++) {
    addLabel(String(r + 1));
    for (let c = 0; c < BOARD_SIZE; c++) {
      const cell = document.createElement('div');
      cell.classList.add('cell');
      cell.title = `${COLUMN_LABELS[c]}${r + 1}`;
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

undoButton.addEventListener('click', onUndo);
redoButton.addEventListener('click', onRedo);
copyButton.addEventListener('click', onCopyMoves);

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
