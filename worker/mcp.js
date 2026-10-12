// An MCP server for playing Complete the Square against Claude: a connector.
//
//   POST /mcp  one JSON-RPC message per request, answered as JSON (no sessions, no SSE)
//
// Claude picks its own moves; this server only knows the rules. The one tool,
// `play`, takes the whole game so far, checks every move with the same
// ai.wasm the page uses, and returns the position. Its result is drawn by
// board.html (an MCP App), which hosts show in the chat. Clicking a square
// there sends the user's move to Claude as a chat message.

import wasm from './ai.wasm';
import boardHtml from './board.html';

const engine = new WebAssembly.Instance(wasm, {}).exports;

const SIZE = 5;
const COLUMNS = 'ABCDE';
const PLAYERS = ['green', 'red'];
const BOARD_URI = 'ui://complete-the-square/board.html';
const PROTOCOL_VERSIONS = ['2025-11-25', '2025-06-18', '2025-03-26', '2024-11-05'];

const RULES = `Rules of Complete the Square:
- A 5x5 board, columns A-E, rows 1-5 with row 1 at the top. Green moves first; the players take turns placing one piece on an empty square.
- Capture: after placing, look along each of the 8 directions. A run of one or more opponent pieces that ends at one of your own pieces is removed. Only the player who moves captures, so placing between two enemy pieces is safe. Captured squares can be played again.
- Win: own all four corners of a square, 2x2 up to 5x5, sides along the rows and columns.
- Draw: the same position for the third time.`;

const PLAY_TOOL = {
  name: 'play',
  title: 'Play Complete the Square',
  description: `Play a game of Complete the Square against the user, and show the board.

You play one colour and choose your own moves by thinking about the position; the user plays the other colour, by clicking the board or naming a square. Ask the user which colour they want if they have not said.

Call this tool to start a game and after every move the user makes: pass the whole game so far, with the user's move and then your reply appended. To start, pass no moves if the user is Green, or your first move if you are Green. The server checks every move: an illegal move returns an error, and the call must leave the user to move (or end the game). Once the game is over, say who won.

${RULES}`,
  inputSchema: {
    type: 'object',
    properties: {
      moves: { type: 'string', description: 'The game so far, in order, e.g. "1. C3 A1  2. B2 D4" or "C3 A1 B2 D4". Empty for a new game.' },
      claude: { type: 'string', enum: PLAYERS, description: 'The colour you (Claude) play.' },
    },
    required: ['moves', 'claude'],
  },
  annotations: { readOnlyHint: true, openWorldHint: false },
  _meta: { ui: { resourceUri: BOARD_URI } },
};

const BOARD_RESOURCE = {
  uri: BOARD_URI,
  name: 'Complete the Square board',
  mimeType: 'text/html;profile=mcp-app',
  _meta: { ui: { prefersBorder: false } },
};

function squareName(index) {
  return COLUMNS[index % SIZE] + (Math.floor(index / SIZE) + 1);
}

function squaresOf(mask) {
  const names = [];
  for (let i = 0; i < SIZE * SIZE; i++) if (mask & (1 << i)) names.push(squareName(i));
  return names;
}

// "1. C3 A1  2. B2" or "C3, A1, B2" -> square indices; throws on anything else.
function parseMoves(text) {
  const squares = [];
  for (const token of text.trim().split(/[\s,]+/)) {
    if (token === '' || /^\d+\.$/.test(token)) continue;
    const match = /^([A-Ea-e])([1-5])$/.exec(token);
    if (!match) throw new Error(`"${token}" is not a square: use a column A-E and a row 1-5, like C3.`);
    squares.push((Number(match[2]) - 1) * SIZE + COLUMNS.indexOf(match[1].toUpperCase()));
  }
  if (squares.length > 500) throw new Error('That game is too long.');
  return squares;
}

function moveText(squares) {
  const pairs = [];
  for (let i = 0; i < squares.length; i += 2) {
    pairs.push(`${i / 2 + 1}. ${squares.slice(i, i + 2).map(squareName).join(' ')}`);
  }
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

// Replay the game with the rules engine; returns the position, for the board
// view and as text for Claude, or throws with a message Claude can act on.
function replay(squares, claude) {
  engine.reset();
  for (const [i, square] of squares.entries()) {
    const over = engine.winner() >= 0 || engine.draw();
    if (!engine.play(Math.floor(square / SIZE), square % SIZE)) {
      const why = over ? 'the game was already over' : 'that square is occupied';
      throw new Error(`Move ${i + 1} (${squareName(square)}) is illegal: ${why}. The position before it:\n${asciiBoard(engine.board(0), engine.board(1))}`);
    }
  }
  const ply = engine.move_count();
  const green = engine.board(0);
  const red = engine.board(1);
  const mover = ply % 2 === 1 ? 0 : 1; // who played the last move
  const captured = ply ? engine.board_at(ply - 1, 1 - mover) & ~engine.board(1 - mover) : 0;
  const winner = engine.winner();
  const draw = engine.draw() === 1;
  const toMove = winner >= 0 || draw ? null : PLAYERS[engine.current_player()];
  if (toMove === claude) {
    throw new Error(`It is your move (${claude}): append your reply to the game and call play again.\n${asciiBoard(green, red)}`);
  }
  const game = {
    moves: squares.map(squareName),
    green: squaresOf(green),
    red: squaresOf(red),
    last: ply ? squareName(squares[ply - 1]) : null,
    captured: squaresOf(captured),
    toMove,
    winner: winner >= 0 ? PLAYERS[winner] : null,
    winningSquare: squaresOf(engine.winning_mask()),
    draw,
    claude,
  };
  return { game, text: describe(game, asciiBoard(green, red), moveText(squares)) };
}

function describe(game, board, moves) {
  let status;
  if (game.winner) status = `${game.winner === game.claude ? 'Claude' : 'The user'} (${game.winner}) wins: ${game.winningSquare.join(' ')}.`;
  else if (game.draw) status = 'Draw: the same position for the third time.';
  else status = `The user (${game.toMove}) to move.`;
  const captured = game.captured.length ? `\nThe last move captured ${game.captured.join(' ')}.` : '';
  return `${board}\n\nMoves: ${moves || '(none yet)'}${captured}\n${status}`;
}

function callTool(name, args) {
  if (name !== 'play') throw rpcError(-32602, `Unknown tool: ${name}`);
  if (typeof args?.moves !== 'string' || !PLAYERS.includes(args?.claude)) {
    return { content: [{ type: 'text', text: 'Pass "moves" (a string) and "claude" ("green" or "red").' }], isError: true };
  }
  try {
    const { game, text } = replay(parseMoves(args.moves), args.claude);
    return { content: [{ type: 'text', text }], structuredContent: game };
  } catch (error) {
    return { content: [{ type: 'text', text: error.message }], isError: true };
  }
}

function rpcError(code, message) {
  return Object.assign(new Error(message), { code });
}

function handle(message) {
  const params = message.params ?? {};
  switch (message.method) {
    case 'initialize':
      return {
        protocolVersion: PROTOCOL_VERSIONS.includes(params.protocolVersion) ? params.protocolVersion : PROTOCOL_VERSIONS[1],
        capabilities: { tools: {}, resources: {} },
        serverInfo: { name: 'complete-the-square', title: 'Complete the Square', version: '1.0.0' },
        instructions: 'Play Complete the Square, a 5x5 board game, against the user: call the play tool to start a game and after each of their moves.',
      };
    case 'ping':
      return {};
    case 'tools/list':
      return { tools: [PLAY_TOOL] };
    case 'tools/call':
      return callTool(params.name, params.arguments);
    case 'resources/list':
      return { resources: [BOARD_RESOURCE] };
    case 'resources/read':
      if (params.uri !== BOARD_URI) throw rpcError(-32002, `Unknown resource: ${params.uri}`);
      return { contents: [{ ...BOARD_RESOURCE, text: boardHtml }] };
    default:
      throw rpcError(-32601, `Unknown method: ${message.method}`);
  }
}

const HEADERS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Headers': 'Content-Type, Accept, Mcp-Protocol-Version, Mcp-Session-Id',
  'Access-Control-Allow-Methods': 'POST, OPTIONS',
};

function json(body, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { ...HEADERS, 'Content-Type': 'application/json' } });
}

export async function handleMcp(request) {
  if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: HEADERS });
  if (request.method !== 'POST') return new Response('POST JSON-RPC messages here.', { status: 405, headers: { ...HEADERS, Allow: 'POST' } });

  let message;
  try {
    message = JSON.parse(await request.text());
  } catch {
    return json({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } }, 400);
  }
  if (Array.isArray(message) || typeof message !== 'object' || message === null) {
    return json({ jsonrpc: '2.0', id: null, error: { code: -32600, message: 'Send one JSON-RPC message per request.' } }, 400);
  }
  // Notifications and replies to us need no answer.
  if (message.id === undefined || message.method === undefined) return new Response(null, { status: 202, headers: HEADERS });

  try {
    return json({ jsonrpc: '2.0', id: message.id, result: handle(message) });
  } catch (error) {
    if (error.code === undefined) throw error;
    return json({ jsonrpc: '2.0', id: message.id, error: { code: error.code, message: error.message } });
  }
}
