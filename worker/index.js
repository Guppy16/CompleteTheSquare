// Stores finished games sent by the page, in D1 (Cloudflare's SQLite), and
// serves the MCP connector for playing against Claude (mcp.js).
//
//   POST /games  {"id": "...", "moves": "1. A1 B2  2. C3", "result": "green", "mode": "ai", "date": "..."}
//
// Only the game is stored: no IP, no user ID. The rate limiter counts requests
// per IP in memory and never stores the IP. The body is sent as text/plain, so
// the browser needs no preflight request.

import { handleMcp } from './mcp.js';

const RESULTS = ['green', 'red', 'draw'];
const MODES = ['board', 'ai', 'analysis'];
const CORS = { 'Access-Control-Allow-Origin': '*' };

function reply(status, text = '') {
  return new Response(text || null, { status, headers: CORS });
}

export default {
  async fetch(request, env) {
    if (new URL(request.url).pathname === '/mcp') return handleMcp(request);
    if (request.method !== 'POST' || new URL(request.url).pathname !== '/games') return reply(404, 'not found');

    const { success } = await env.LIMITER.limit({ key: request.headers.get('CF-Connecting-IP') || 'unknown' });
    if (!success) return reply(429, 'too many games');

    let game;
    try {
      game = JSON.parse(await request.text());
    } catch {
      return reply(400, 'not JSON');
    }
    const ok =
      typeof game.id === 'string' && /^[0-9a-f-]{36}$/.test(game.id) &&
      typeof game.moves === 'string' && game.moves.length <= 2000 && /^[0-9A-E. ]*$/.test(game.moves) &&
      RESULTS.includes(game.result) &&
      MODES.includes(game.mode) &&
      typeof game.date === 'string' && !Number.isNaN(Date.parse(game.date));
    if (!ok) return reply(400, 'not a game');

    // A retried game (its reply was lost) has the same id and is ignored.
    await env.DB.prepare('INSERT OR IGNORE INTO games (id, moves, result, mode, played_at) VALUES (?, ?, ?, ?, ?)')
      .bind(game.id, game.moves, game.result, game.mode, game.date)
      .run();
    return reply(204);
  },
};
