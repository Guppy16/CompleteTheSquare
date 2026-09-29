// Complete the Square: page logic.
//
// All game state and rules live in the WebAssembly module (built from wasm/).
// This file only forwards clicks to it and redraws the board from its state.
// The engine's searches run in a Web Worker (worker.js) so the page never
// blocks.

const BOARD_SIZE = 5;
// Against the AI: search deeper and deeper until AI_NODE_BUDGET positions
// have been visited (about 70 ms on a laptop, a few hundred ms on a phone).
const AI_MAX_DEPTH = 12;
const AI_NODE_BUDGET = 400000;
// Analysis: keep re-analysing with a doubling budget, like an engine that
// keeps thinking, for a few seconds (ANALYSIS_BUDGET nodes); "Go deeper"
// continues for another few seconds. The worker keeps its table, so each
// pass (and the analysis after the next move) builds on the last one.
const ANALYSIS_MAX_DEPTH = 12;
const ANALYSIS_FIRST_BUDGET = 500000;
const ANALYSIS_BUDGET = 8000000;       // roughly 3-5 s in the browser
const ANALYSIS_LINES = 3;              // how many candidate lines to show
const EVALUATE_BUDGET = 150000;        // per position, for the scores next to each move
const WASM_URL = 'square-game/ai.wasm';
const WORKER_URL = 'square-game/worker.js';
const COLOURS = ['W', 'B'];            // player 0 is green (W), player 1 is red (B)
const COLUMN_LABELS = 'ABCDEFGHIJ';     // columns are lettered, rows numbered from the top

let ai = null;                         // the module's exports, once loaded
let mode = 'board';                    // 'board' (two humans), 'ai', or 'analysis'
let aiPlayer = 1;                      // which side the AI plays in AI mode (0 = green, first)
let analysisGeneration = 0;            // bumped whenever the position or mode changes

const boardEl = document.getElementById('board');
const statusEl = document.getElementById('status');
const playAgainButton = document.getElementById('play-again');
const undoButton = document.getElementById('undo');
const redoButton = document.getElementById('redo');
const copyButton = document.getElementById('copy-moves');
const hintButton = document.getElementById('hint');
const pasteButton = document.getElementById('paste-moves');
const moveListEl = document.getElementById('move-list');
const analysisEl = document.getElementById('analysis');
const evalColumn = document.getElementById('eval-column');
const evalFill = document.getElementById('eval-fill');
const evalText = document.getElementById('eval-text');
const sideSelect = document.getElementById('side-select');
const aiFirstBox = document.getElementById('ai-first');
const tabs = Array.from(document.querySelectorAll('#mode-select .tab'));
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
  return mode === 'ai' && !gameOver() && ai.current_player() === aiPlayer;
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
// of the plies in `lastMoves` in the brighter shade.
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
// the position after them.
function renderMoves() {
  moveListEl.innerHTML = '';
  const count = ai.move_count();
  moveListEl.hidden = count === 0;     // nothing to show before the first move
  if (count === 0) return;
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
    // In analysis: the evaluation after this move, once the game has been scored.
    const evalScore = gameScores.get(plies.length + i);
    if (mode === 'analysis' && evalScore !== undefined) {
      const evalEl = document.createElement('span');
      evalEl.classList.add('eval');
      evalEl.textContent = scoreText(evalScore);
      pair.appendChild(evalEl);
    }
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
    cell.textContent = '';             // drops any analysis marker
    boards.forEach((board, player) => {
      if (board & bit) cell.classList.add(`player-${COLOURS[player]}`, 'disabled');
    });
    if (over) cell.classList.add('disabled');
    if (winMask & bit) cell.classList.add('winner');
  });
  const count = ai.move_count();
  if (count > 0) cells[ai.move_at(count - 1)].classList.add('last');

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
  hintButton.hidden = mode === 'analysis';
  hintButton.disabled = waiting || over;
  renderMoves();
  boardEl.classList.remove('thinking');
  startAnalysis();                     // no-op unless in analysis mode
}

function logMoves() {
  console.log(moveText());
}

// --- Engine requests -------------------------------------------------------
//
// askEngine() sends the moves played so far to the worker and resolves with
// its answer: a square to play ('move'), or every move scored with its
// expected line ('analyse'). Falls back to the main thread without workers.
const engineWorker = typeof Worker === 'undefined' ? null : new Worker(WORKER_URL);
const engineRequests = new Map();       // request id -> resolve
let engineRequestId = 0;

if (engineWorker) {
  engineWorker.onmessage = event => {
    const resolve = engineRequests.get(event.data.id);
    engineRequests.delete(event.data.id);
    if (resolve) resolve(event.data);
  };
  engineWorker.onerror = err => console.error('engine worker failed:', err);
}

function askEngine(kind, maxDepth, nodeBudget) {
  const moves = [];
  for (let i = 0; i < ai.move_count(); i++) moves.push(ai.move_at(i));
  return askEngineAt(moves, kind, maxDepth, nodeBudget);
}

// The same for the position after the given moves (used to score a whole
// game). The fallback without a worker can only see the current position.
function askEngineAt(moves, kind, maxDepth, nodeBudget) {
  if (!engineWorker) {
    const started = performance.now();
    if (kind === 'analyse') {
      const count = ai.analyse(maxDepth, nodeBudget);
      const lines = [];
      for (let i = 0; i < count; i++) {
        const line = [];
        for (let j = 0; ; j++) {
          const square = ai.analysis_line(i, j);
          if (square < 0) break;
          line.push(square);
        }
        lines.push({ index: ai.analysis_move(i), score: ai.analysis_score(i), line });
      }
      return Promise.resolve({ lines, depth: ai.analysis_depth(), book: ai.book_square(), ms: Math.round(performance.now() - started) });
    }
    if (kind === 'evaluate') {
      return Promise.resolve({ score: ai.evaluate(maxDepth, nodeBudget), ms: Math.round(performance.now() - started) });
    }
    const index = ai.ai_suggest(maxDepth, nodeBudget);
    return Promise.resolve({ index, ms: Math.round(performance.now() - started) });
  }
  const id = ++engineRequestId;
  return new Promise(resolve => {
    engineRequests.set(id, resolve);
    engineWorker.postMessage({ id, kind, moves, boardSize: BOARD_SIZE, maxDepth, nodeBudget });
  });
}

// --- Analysis ----------------------------------------------------------------
//
// Scores are shown for Green (like lichess shows White), whoever is to move.
function greenScore(score) {
  return ai.current_player() === 0 ? score : -score;
}

function scoreText(green) {
  if (green >= 1) return 'Green wins';
  if (green <= -1) return 'Red wins';
  return (green >= 0 ? '+' : '') + green.toFixed(2);
}

// A line as lichess writes it: "3. C3 D4 4. B1", or "3... D4 4. B1" when it
// starts with the second player's move. `ply` is how many moves precede it.
function lineText(squares, ply) {
  const parts = [];
  squares.forEach((square, i) => {
    const p = ply + i;
    if (p % 2 === 0) parts.push(`${p / 2 + 1}.`);
    else if (i === 0) parts.push(`${Math.floor(p / 2) + 1}...`);
    parts.push(squareName(square));
  });
  return parts.join(' ');
}

let analysisBudget = 0;                // budget of the last completed pass
let analysisCap = ANALYSIS_BUDGET;     // "Go deeper" raises it
let analysisRunning = false;

// Analyse the current position with a doubling budget up to the cap, showing
// each pass as it lands. Stops when the position or mode changes.
function startAnalysis() {
  analysisGeneration += 1;
  const inAnalysis = mode === 'analysis';
  evalColumn.hidden = !inAnalysis;
  analysisEl.hidden = !inAnalysis;     // stays in place even when the game is over
  if (!inAnalysis) return;
  if (gameOver()) {
    analysisEl.textContent = ai.winner() >= 0 ? `${getColorName(ai.winner())} won.` : 'Drawn.';
    evaluateGame();
    return;
  }
  analysisEl.textContent = 'Analysing…';
  analysisBudget = 0;
  analysisCap = ANALYSIS_BUDGET;
  continueAnalysis(ANALYSIS_FIRST_BUDGET);
  evaluateGame();
}

function continueAnalysis(budget) {
  const generation = analysisGeneration;
  const snapshot = moveText();
  analysisRunning = true;
  askEngine('analyse', ANALYSIS_MAX_DEPTH, budget).then(({ lines, depth, book }) => {
    if (generation !== analysisGeneration || moveText() !== snapshot) return;   // position moved on
    analysisRunning = false;
    analysisBudget = budget;
    const settled = lines.length === 0 || Math.abs(lines[0].score) >= 1 || depth >= ANALYSIS_MAX_DEPTH;
    renderAnalysis(lines, depth, settled, book);
    if (!settled && budget < analysisCap) continueAnalysis(budget * 2);
  });
}

function goDeeper() {
  analysisCap = analysisBudget * 8;    // three more doublings: a few more seconds
  if (!analysisRunning) continueAnalysis(analysisBudget * 2);
}

function renderAnalysis(lines, depth, settled, book = -1) {
  const top = lines.slice(0, ANALYSIS_LINES);
  const best = top.length ? greenScore(top[0].score) : 0;
  const ply = ai.move_count();
  const toMove = ai.current_player();

  // Eval bar: Green's share, forced wins fill it completely.
  const clamped = Math.max(-1, Math.min(1, best));
  evalFill.style.height = `${50 + 50 * clamped}%`;
  evalText.textContent = scoreText(best);

  // The book move, if any, is shown first: it comes from a depth-14 offline
  // search, deeper than the analysis reaches in a few seconds.
  const bookLine = book >= 0 ? lines.find(l => l.index === book) : undefined;
  const shown = bookLine ? [bookLine, ...top.filter(l => l.index !== book)] : top;

  // Candidate markers on the board: size and opacity shrink with the gap to
  // the best engine move; the book move is always full size.
  cells.forEach(cell => cell.textContent = '');
  shown.forEach(({ index, score }) => {
    const gap = index === book ? 0 : Math.abs(top[0].score - score);
    const size = Math.max(0.55, 1 - gap * 4);          // 0.11 behind: about half size
    const opacity = Math.max(0.45, 1 - gap * 3);
    const mark = document.createElement('span');
    mark.classList.add('mark', toMove === 0 ? 'green' : 'red');
    if (index === book) mark.classList.add('book');
    mark.style.width = `${Math.round(72 * size)}%`;
    mark.style.height = `${Math.round(72 * size)}%`;
    mark.style.fontSize = `${(0.7 * size).toFixed(2)}rem`;
    mark.style.opacity = opacity.toFixed(2);
    mark.textContent = scoreText(greenScore(score));
    cells[index].appendChild(mark);
  });

  // Header: depth, and Go deeper once the automatic passes have finished.
  analysisEl.innerHTML = '';
  const header = document.createElement('div');
  header.classList.add('header');
  const info = document.createElement('span');
  info.textContent = `Depth ${depth} · ${getColorName(toMove)} to move`;
  header.appendChild(info);
  if (!settled && analysisBudget >= analysisCap) {
    const deeper = document.createElement('button');
    deeper.textContent = 'Go deeper';
    deeper.addEventListener('click', goDeeper);
    header.appendChild(deeper);
  } else if (!settled) {
    const thinking = document.createElement('span');
    thinking.textContent = 'thinking…';
    header.appendChild(thinking);
  }
  analysisEl.appendChild(header);

  // The lines: the book move first if there is one, then the engine's best;
  // scores for Green. Tap one to play its first move.
  shown.forEach(({ index, score, line }) => {
    const row = document.createElement('div');
    row.classList.add('line');
    const scoreEl = document.createElement('span');
    scoreEl.classList.add('score');
    scoreEl.textContent = scoreText(greenScore(score));
    const movesEl = document.createElement('span');
    movesEl.classList.add('moves');
    movesEl.textContent = lineText(line, ply);
    row.append(scoreEl, movesEl);
    if (index === book) {
      const tag = document.createElement('span');
      tag.classList.add('tag');
      tag.textContent = 'book';
      tag.title = 'From the opening book: a depth-14 search, deeper than this analysis';
      row.appendChild(tag);
    }
    row.addEventListener('click', () => onCellClick(Math.floor(index / BOARD_SIZE), index % BOARD_SIZE));
    analysisEl.appendChild(row);
  });
}

// Scores after each move of the game, for the move list (like lichess's
// computer analysis). Cached by the move prefix so only new positions are
// scored when the game grows; a quick search per position.
const gameScores = new Map();          // ply -> score for Green after that many moves
let gameScoresPrefix = [];             // the moves those scores belong to

function evaluateGame() {
  const moves = [];
  for (let i = 0; i < ai.move_count(); i++) moves.push(ai.move_at(i));
  // Keep cached scores while the game's moves still match.
  let same = 0;
  while (same < moves.length && same < gameScoresPrefix.length && moves[same] === gameScoresPrefix[same]) same++;
  for (const ply of Array.from(gameScores.keys())) if (ply > same) gameScores.delete(ply);
  gameScoresPrefix = moves;

  const generation = analysisGeneration;
  const pending = [];
  for (let ply = 1; ply <= moves.length; ply++) if (!gameScores.has(ply)) pending.push(ply);
  const next = () => {
    if (generation !== analysisGeneration || pending.length === 0) return;
    const ply = pending.shift();
    if (ply === moves.length && gameOver()) {   // the final position needs no search
      const winner = ai.winner();
      gameScores.set(ply, winner < 0 ? 0 : winner === 0 ? 1 : -1);
      renderMoves();
      next();
      return;
    }
    askEngineAt(moves.slice(0, ply), 'evaluate', AI_MAX_DEPTH, EVALUATE_BUDGET).then(({ score }) => {
      if (generation !== analysisGeneration) return;
      const toMove = ply % 2;          // after an odd number of moves, red is to move
      gameScores.set(ply, toMove === 0 ? score : -score);
      renderMoves();
      next();
    });
  };
  next();
}

// --- Playing -----------------------------------------------------------------

// Ask the engine for the AI's move and play it, unless the game moved on meanwhile.
function requestAIMove() {
  statusEl.textContent = 'AI is thinking…';
  boardEl.classList.add('thinking');
  undoButton.disabled = true;
  redoButton.disabled = true;
  const snapshot = moveText();
  askEngine('move', AI_MAX_DEPTH, AI_NODE_BUDGET).then(({ index, ms }) => {
    if (moveText() !== snapshot || !isAITurn() || index < 0) {
      render();                          // stale answer: the game changed while thinking
      return;
    }
    ai.play(Math.floor(index / BOARD_SIZE), index % BOARD_SIZE);
    console.log(`AI played ${squareName(index)} in ${ms} ms`);
    logMoves();
    render();
  });
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
  if (mode === 'ai' && ai.current_player() === aiPlayer) ai.undo();
  render();
  if (isAITurn()) requestAIMove();     // never leave the game waiting on the AI
}

function onRedo() {
  if (!ai || isAITurn()) return;
  if (!ai.redo()) return;
  if (mode === 'ai' && ai.current_player() === aiPlayer && ai.redo_count() > 0) ai.redo();
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
  hintButton.disabled = true;
  const snapshot = moveText();
  askEngine('move', AI_MAX_DEPTH, AI_NODE_BUDGET).then(({ index }) => {
    hintButton.disabled = false;
    if (index < 0 || moveText() !== snapshot || gameOver()) return;
    cells.forEach(cell => cell.classList.remove('hint'));
    cells[index].classList.add('hint');
    statusEl.innerHTML = createColoredStatus(ai.current_player()) + ` <small>(engine suggests ${squareName(index)})</small>`;
  });
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

// Switch mode, keeping the current game (so a finished game can be analysed).
function setMode(next) {
  mode = next;
  tabs.forEach(tab => tab.classList.toggle('active', tab.dataset.mode === mode));
  sideSelect.hidden = mode !== 'ai';
  if (!ai) return;
  render();
  if (isAITurn()) requestAIMove();
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
  addLabel('');                        // right-hand spacer keeps the squares centred
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
    addLabel('');
  }
}

tabs.forEach(tab => tab.addEventListener('click', () => setMode(tab.dataset.mode)));

aiFirstBox.addEventListener('change', () => {
  aiPlayer = aiFirstBox.checked ? 0 : 1;
  sideSelect.querySelector('.left').classList.toggle('active', !aiFirstBox.checked);
  sideSelect.querySelector('.right').classList.toggle('active', aiFirstBox.checked);
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
