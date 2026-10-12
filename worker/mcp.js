// An MCP server for playing Complete the Square against Claude: a connector.
//
//   POST /mcp  one JSON-RPC message per request, answered as JSON (no sessions, no SSE)
//
// Claude picks its own moves; the server keeps each game (a Game Durable
// Object, game.js) and checks every move. A whole game is one turn of Claude's:
//
//   new_game -> play (Claude's move) -> wait_for_move (until the user clicks) -> play -> ...
//
// new_game's result is drawn by board.html (an MCP App) in the chat, one board
// for the whole game. A click there calls user_move, which wakes the waiting
// wait_for_move, so the user never has to send a message; the board follows
// Claude's moves through game_state, a tool only the board sees.

import boardHtml from './board.html';
import { PLAYERS, other, parseSquare } from './rules.js';

// Hosts cache a view by its address, so the address carries a hash of the
// HTML: a changed board gets a new address (once the host reloads the tool list).
const BOARD_URI = `ui://complete-the-square/board-${hash(boardHtml)}.html`;
const OLD_BOARDS = 'ui://complete-the-square/board';
// FNV-1a: a short hash of a string, as 8 hex digits.
function hash(text) {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193);
  return (h >>> 0).toString(16).padStart(8, '0');
}

const PROTOCOL_VERSIONS = ['2025-11-25', '2025-06-18', '2025-03-26', '2024-11-05'];

const RULES = `Rules of Complete the Square:
- A 5x5 board, columns A-E, rows 1-5 with row 1 at the top. Green moves first; the players take turns placing one piece on an empty square.
- Capture: after placing, look along each of the 8 directions. A run of one or more opponent pieces that ends at one of your own pieces is removed. Only the player who moves captures, so placing between two enemy pieces is safe. Captured squares can be played again.
- Win: own all four corners of a square, 2x2 up to 5x5, sides along the rows and columns.
- Draw: the same position for the third time.`;

const HOW_TO_PLAY = `You play one colour and choose your own moves by thinking about the position; the user plays the other, by clicking the board. Ask which colour they want if they have not said.

Keep the game going within your turn: after each of your moves, call wait_for_move, and call it again whenever it says the user has not moved yet. Never ask the user to send a message to continue.

Play like an opponent across the board: say your move in a sentence, without analysing the position for the user, pointing out their threats or good moves, or explaining your plans. When the game ends, say who won.`;

const GAME_ID = { type: 'string', description: 'The game id from new_game.' };
const SQUARE = { type: 'string', description: 'A square: a column A-E and a row 1-5, like C3.' };

const TOOLS = [
  {
    name: 'new_game',
    title: 'New game of Complete the Square',
    description: `Start a game of Complete the Square against the user, and show the board, which follows the game from then on.\n\n${HOW_TO_PLAY}\n\n${RULES}`,
    inputSchema: {
      type: 'object',
      properties: { claude: { type: 'string', enum: PLAYERS, description: 'The colour you (Claude) play. Green moves first.' } },
      required: ['claude'],
    },
    _meta: { ui: { resourceUri: BOARD_URI } },
  },
  {
    name: 'play',
    title: 'Play a move',
    description: "Play your move in a game; the board from new_game shows it. Then call wait_for_move.",
    inputSchema: { type: 'object', properties: { game: GAME_ID, move: SQUARE }, required: ['game', 'move'] },
  },
  {
    name: 'wait_for_move',
    title: "Wait for the user's move",
    description: "Wait for the user to play on the board, up to about 45 seconds. Returns their move and the position, or says they have not moved yet: then call it again.",
    inputSchema: { type: 'object', properties: { game: GAME_ID }, required: ['game'] },
    annotations: { readOnlyHint: true },
  },
  {
    name: 'user_move',
    title: "Play the user's move",
    description: "Play the user's move. The board calls this when the user clicks; call it yourself only when the user names their move in the chat instead. Never choose a move for the user.",
    inputSchema: { type: 'object', properties: { game: GAME_ID, move: SQUARE }, required: ['game', 'move'] },
    _meta: { ui: { visibility: ['model', 'app'] } },
  },
  {
    name: 'game_state',
    title: 'Game state',
    description: 'For the board: the game once it has more than `after` moves (or has ended), waiting up to about 45 seconds; at once without `after`.',
    inputSchema: { type: 'object', properties: { game: GAME_ID, after: { type: 'integer', minimum: 0 } }, required: ['game'] },
    annotations: { readOnlyHint: true },
    _meta: { ui: { visibility: ['app'] } },
  },
];

const BOARD_RESOURCE = {
  uri: BOARD_URI,
  name: 'Complete the Square board',
  mimeType: 'text/html;profile=mcp-app',
  _meta: { ui: { prefersBorder: false } },
};

// The game as text for Claude, ending with what to do next.
function describe(game, gameId, next) {
  const lines = [game.board, '', `Moves: ${game.moveText || '(none yet)'}`];
  if (game.captured.length) lines.push(`The last move captured ${game.captured.join(' ')}.`);
  if (game.winner) lines.push(`Game over: ${game.winner === game.claude ? 'you' : 'the user'} (${game.winner}) won with ${game.winningSquare.join(' ')}.`);
  else if (game.draw) lines.push('Game over: a draw, the same position for the third time.');
  else lines.push(next);
  lines.push(`Game id: ${gameId}`);
  return lines.join('\n');
}

function nextStep(game) {
  return game.toMove === game.claude ? `Your move (${game.claude}): call play.` : `The user (${other(game.claude)}) to move: call wait_for_move.`;
}

async function callTool(name, args, env) {
  if (name === 'new_game') {
    if (!PLAYERS.includes(args?.claude)) throw new Error('Pass "claude": "green" or "red".');
    const id = env.GAMES.newUniqueId();
    const game = await env.GAMES.get(id).start(args.claude);
    return result(game, id.toString(), nextStep(game));
  }
  const gameId = String(args?.game ?? '');
  let stub;
  try {
    stub = env.GAMES.get(env.GAMES.idFromString(gameId));
  } catch {
    throw new Error(`"${gameId}" is not a game id: start a game with new_game.`);
  }
  if (name === 'play') {
    const game = await stub.move('claude', parseSquare(args.move));
    return result(game, gameId, nextStep(game));
  }
  if (name === 'user_move') {
    const game = await stub.move('user', parseSquare(args.move));
    return result(game, gameId, nextStep(game));
  }
  if (name === 'game_state') {
    const game = await stub.watch(Number.isInteger(args.after) ? args.after : undefined);
    return result(game, gameId, nextStep(game));
  }
  if (name === 'wait_for_move') {
    const game = await stub.waitForUser();
    const next = game.toMove === game.claude
      ? `The user played ${game.last}. Your move (${game.claude}): call play.`
      : 'The user has not moved yet: call wait_for_move again.';
    return result(game, gameId, next);
  }
  throw rpcError(-32602, `Unknown tool: ${name}`);
}

function result(game, gameId, next) {
  const { board, moveText, ...shown } = game;
  return { content: [{ type: 'text', text: describe(game, gameId, next) }], structuredContent: { ...shown, game: gameId } };
}

function rpcError(code, message) {
  return Object.assign(new Error(message), { code });
}

async function handle(message, env) {
  const params = message.params ?? {};
  switch (message.method) {
    case 'initialize':
      return {
        protocolVersion: PROTOCOL_VERSIONS.includes(params.protocolVersion) ? params.protocolVersion : PROTOCOL_VERSIONS[1],
        capabilities: { tools: {}, resources: {} },
        serverInfo: { name: 'complete-the-square', title: 'Complete the Square', version: '2.0.0' },
        instructions: `Play Complete the Square, a 5x5 board game, against the user: start with new_game.\n\n${HOW_TO_PLAY}`,
      };
    case 'ping':
      return {};
    case 'tools/list':
      return { tools: TOOLS };
    case 'tools/call':
      // A rule broken, or a game that does not exist, is a tool error that Claude can act on.
      try {
        return await callTool(params.name, params.arguments, env);
      } catch (error) {
        if (error.code !== undefined) throw error;
        return { content: [{ type: 'text', text: error.message }], isError: true };
      }
    case 'resources/list':
      return { resources: [BOARD_RESOURCE] };
    case 'resources/read':
      // An older board's address gets the current board.
      if (!String(params.uri).startsWith(OLD_BOARDS)) throw rpcError(-32002, `Unknown resource: ${params.uri}`);
      return { contents: [{ ...BOARD_RESOURCE, uri: params.uri, text: boardHtml }] };
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

export async function handleMcp(request, env) {
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

  // One line per request in the Worker's logs: the method, the tool and how it went.
  const started = Date.now();
  const what = [message.method, message.params?.name, JSON.stringify(message.params?.arguments ?? '')].filter(Boolean).join(' ');
  try {
    const result = await handle(message, env);
    console.log(`${what} -> ${result?.isError ? `tool error: ${result.content[0].text}` : 'ok'} in ${Date.now() - started} ms`);
    return json({ jsonrpc: '2.0', id: message.id, result });
  } catch (error) {
    console.log(`${what} -> threw ${error.message} after ${Date.now() - started} ms`);
    if (error.code === undefined) throw error;
    return json({ jsonrpc: '2.0', id: message.id, error: { code: error.code, message: error.message } });
  }
}
