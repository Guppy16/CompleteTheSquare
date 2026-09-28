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
let aiPlayer = 1;                      // which side the AI plays in AI mode (0 = green, first)
const aiFirstBox = document.getElementById('ai-first');
const aiFirstLabel = document.getElementById('ai-first-label');

const boardEl = document.getElementById('board');
const statusEl = document.getElementById('status');
const playAgainButton = document.getElementById('play-again');
const undoButton = document.getElementById('undo');
const redoButton = document.getElementById('redo');
const copyButton = document.getElementById('copy-moves');
const hintButton = document.getElementById('hint');
const pasteButton = document.getElementById('paste-moves');
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
  return playAgainstAI && !gameOver() && ai.current_player() === aiPlayer;
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

// A tiny picture of the position after `ply` half-moves, with the squares
// of the plies in `lastMoves` outlined.
function miniBoard(ply, lastMoves) {
  const mini = document.createElement('span');
  mini.classList.add('mini-board');
  const boards = [ai.board_at(ply, 0), ai.board_at(ply, 1)];
  for (let i = 0; i < BOARD_SIZE * BOARD_SIZE; i++) {
    const cell = document.createElement('span');
    const bit = 1 << i;
    boards.forEach((board, player) => {
      if (board & bit) cell.classList.add(`player-${COLOURS[player]}`);
    });
    if (lastMoves.includes(i)) cell.classList.add('last');
    mini.appendChild(cell);
  }
  return mini;
}

// One entry per move (a pair of plies, one per side): "3. C3 D4" followed by
// the position after them with those two squares outlined.
function renderMoves() {
  moveListEl.innerHTML = '';
  const count = ai.move_count();
  if (count === 0) {
    moveListEl.textContent = 'No moves yet';
    return;
  }
  for (let i = 0; i < count; i += 2) {
    const plies = [i, i + 1].filter(p => p < count);
    const squares = plies.map(p => ai.move_at(p));

    const pair = document.createElement('span');
    pair.classList.add('pair');
    const number = document.createElement('span');
    number.classList.add('num');
    number.textContent = `${i / 2 + 1}.`;
    const text = document.createElement('span');
    text.classList.add('plies');
    text.textContent = squares.map(squareName).join(' ');
    pair.append(number, text, miniBoard(plies.length + i, squares));
    moveListEl.appendChild(pair);
  }
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
  pasteButton.disabled = waiting;
  hintButton.disabled = waiting || over;
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
  if (playAgainstAI && ai.current_player() === aiPlayer) ai.undo();
  render();
  if (isAITurn()) requestAIMove();     // never leave the game waiting on the AI
}

function onRedo() {
  if (!ai || isAITurn()) return;
  if (!ai.redo()) return;
  if (playAgainstAI && ai.current_player() === aiPlayer && ai.redo_count() > 0) ai.redo();
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

// Highlight the square the engine would play, without playing it.
function onHint() {
  if (!ai || isAITurn() || gameOver()) return;
  const index = ai.ai_suggest(AI_MAX_DEPTH, AI_NODE_BUDGET);
  if (index < 0) return;
  cells.forEach(cell => cell.classList.remove('hint'));
  cells[index].classList.add('hint');
  statusEl.innerHTML = createColoredStatus(ai.current_player()) + ` <small>(engine suggests ${squareName(index)})</small>`;
}

// "1. C3 A1  2. B2 D4" -> [[2, 2], [0, 0], [1, 1], [3, 3]] as [row, col]; null if malformed.
function parseMoves(text) {
  const moves = [];
  for (const token of text.trim().split(/\s+/)) {
    if (token === '' || token.endsWith('.')) continue;     // skip move numbers
    const col = COLUMN_LABELS.indexOf(token[0].toUpperCase());
    const row = parseInt(token.slice(1), 10) - 1;
    if (col < 0 || col >= BOARD_SIZE || !(row >= 0 && row < BOARD_SIZE)) return null;
    moves.push([row, col]);
  }
  return moves;
}

// Start a new game from a pasted move list (the format "Copy moves" produces).
function onPasteMoves() {
  if (!ai || isAITurn()) return;
  const text = window.prompt('Paste moves, e.g. "1. C3 A1  2. B2 D4":');
  if (text === null) return;
  const moves = parseMoves(text);
  if (moves === null) {
    statusEl.textContent = 'Could not read those moves.';
    return;
  }
  ai.reset();
  for (const [i, [row, col]] of moves.entries()) {
    if (!ai.play(row, col)) {
      render();
      statusEl.textContent = `Move ${i + 1} (${squareName(row * BOARD_SIZE + col)}) is not legal here; stopped before it.`;
      return;
    }
  }
  render();
  logMoves();
  if (isAITurn()) requestAIMove();
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
  if (isAITurn()) requestAIMove();     // the AI opens when it plays first
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
  aiFirstLabel.hidden = !playAgainstAI;
  if (ai) newGame();
});

aiFirstBox.addEventListener('change', () => {
  aiPlayer = aiFirstBox.checked ? 0 : 1;
  if (ai) newGame();
});

playAgainButton.addEventListener('click', () => {
  if (ai) newGame();
});

undoButton.addEventListener('click', onUndo);
redoButton.addEventListener('click', onRedo);
copyButton.addEventListener('click', onCopyMoves);
pasteButton.addEventListener('click', onPasteMoves);
hintButton.addEventListener('click', onHint);

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
