// Online games between two people, over WebSockets.
//
// A Room is one game: a Durable Object that holds both players' sockets,
// checks every move with the rules, and sends the new state to everyone
// connected (a third visitor watches). Every game is rated. A friend game
// starts from a link; a quick match from the Lobby, the one Durable Object
// that pairs waiting players of similar rating. Rooms hibernate between messages, so a game that waits
// for a move costs nothing.
//
// A player is a random id and a secret key, made by the browser and kept in its
// localStorage. Only a hash of the key is stored, in the players table (D1),
// with the player's rating. Nothing else identifies a player.

import { DurableObject } from 'cloudflare:workers';
import { PROVISIONAL_RD, START, aged, glicko2 } from './ratings.js';
import { PLAYERS, other, position } from './rules.js';

// A Room is deleted this long after its last change.
const KEEP_MS = 7 * 24 * 60 * 60_000;

// A player whose opponent has been gone this long may claim the game.
export const ABANDON_MS = 60_000;

// Messages a minute one room takes from all its sockets together.
const MESSAGES_PER_MINUTE = 120;

// The lobby pairs two players whose ratings are within the wider of their two
// windows; a window widens the longer its player waits, so nobody waits for long.
const WINDOW = 100;
const WINDOW_PER_SECOND = 25;
const MATCH_EVERY_MS = 2000;

// Game codes: 8 characters without look-alikes (no 0/O, 1/I/L).
const CODE_ALPHABET = 'abcdefghjkmnpqrstuvwxyz23456789';
export const CODE_PATTERN = /^[a-hjkmnp-z2-9]{8}$/;

export function newCode() {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  return Array.from(bytes, (b) => CODE_ALPHABET[b % CODE_ALPHABET.length]).join('');
}

export async function hashKey(key) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(key));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

const ID_PATTERN = /^[0-9a-f-]{36}$/;
const KEY_PATTERN = /^[0-9a-f]{32,64}$/;

// The player with this id and key, created on first sight; null if the key is
// wrong. Their deviation has grown with the days since their last rated game.
export async function player(env, id, key) {
  if (!ID_PATTERN.test(String(id)) || !KEY_PATTERN.test(String(key))) return null;
  const keyHash = await hashKey(key);
  await env.DB.prepare('INSERT OR IGNORE INTO players (id, key_hash, rating, rd, vol, games) VALUES (?, ?, ?, ?, ?, 0)')
    .bind(id, keyHash, START.rating, START.rd, START.vol)
    .run();
  const row = await env.DB.prepare('SELECT key_hash, rating, rd, vol, games, last_game_at FROM players WHERE id = ?').bind(id).first();
  if (row.key_hash !== keyHash) return null;
  const days = row.last_game_at ? (Date.now() - Date.parse(row.last_game_at)) / 86_400_000 : 0;
  return { id, keyHash, games: row.games, ...aged(row, days) };
}

// A player's rating now, by id alone (for a game that has already checked the key).
async function ratingOf(env, id) {
  const row = await env.DB.prepare('SELECT rating, rd, vol, games, last_game_at FROM players WHERE id = ?').bind(id).first();
  const days = row.last_game_at ? (Date.now() - Date.parse(row.last_game_at)) / 86_400_000 : 0;
  return { games: row.games, ...aged(row, days) };
}

// What a page is told about a rating: rounded, and whether it is still provisional.
export const shown = ({ rating, rd }) => ({ rating: Math.round(rating), provisional: rd > PROVISIONAL_RD });

const lobby = (env) => env.LOBBY.get(env.LOBBY.idFromName('lobby'));

function send(ws, message) {
  try {
    ws.send(JSON.stringify(message));
  } catch {
    // The socket closed meanwhile; its close event tidies up.
  }
}

// Answer a close from the other end (it may already be closed).
function closeQuietly(ws) {
  try {
    ws.close(1000);
  } catch {
    // already closed
  }
}

// Pages send "ping" every so often so that no network drops an idle socket;
// Cloudflare answers "pong" without waking the Durable Object.
function answerPings(ctx) {
  ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'));
}

export class Room extends DurableObject {
  messages = [];                             // when the messages of the last minute came

  constructor(ctx, env) {
    super(ctx, env);
    answerPings(ctx);
  }

  // A new game: `seats` maps a colour to a player (see player()) or
  // null for a seat the first visitor with the link takes.
  async create(seats) {
    if (await this.ctx.storage.get('seats')) throw new Error('This room is taken.');
    await this.ctx.storage.put({
      seats, game: crypto.randomUUID(), moves: [], over: null,
      undo: null, rematch: [], left: {}, ratingChange: null, changedAt: Date.now(),
    });
    await this.ctx.storage.setAlarm(Date.now() + KEEP_MS);
  }

  async alarm() {
    await this.close('This game has expired.');
  }

  // Delete the game and close its sockets: it expired, or was cancelled before anyone joined.
  async close(reason) {
    for (const ws of this.ctx.getWebSockets()) ws.close(1000, reason);
    await lobby(this.env).playing(this.ctx.id.toString(), []);
    await this.ctx.storage.deleteAlarm();
    await this.ctx.storage.deleteAll();
  }

  // A socket for player `id` with `key`: their seat, a free seat, or a seat to watch from.
  async fetch(request) {
    if (request.headers.get('Upgrade') !== 'websocket') return new Response('expected a WebSocket', { status: 426 });
    const params = new URL(request.url).searchParams;
    const id = params.get('player');
    const key = params.get('key') ?? '';
    const keyHash = KEY_PATTERN.test(key) ? await hashKey(key) : null;

    // Find the player's seat, or give them the free one. Nothing else runs in
    // between, so two people opening the link at once cannot both take it.
    let colour = null;
    let exists = true;
    await this.ctx.blockConcurrencyWhile(async () => {
      const seats = await this.ctx.storage.get('seats');
      if (!seats) {
        exists = false;
        return;
      }
      colour = PLAYERS.find((c) => seats[c]?.id === id && seats[c].keyHash === keyHash) ?? null;
      const free = PLAYERS.find((c) => seats[c] === null);
      if (!colour && free && keyHash && ID_PATTERN.test(String(id)) && !PLAYERS.some((c) => seats[c]?.id === id)) {
        const joining = await player(this.env, id, key);
        if (joining) {
          seats[free] = joining;
          colour = free;
          await this.ctx.storage.put('seats', seats);
        }
      }
    });
    if (!exists) return new Response('There is no such game, or it has expired.', { status: 404 });

    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server);
    server.serializeAttachment({ colour });
    if (colour) await this.reportPlayers();
    if (colour) {
      const left = await this.ctx.storage.get('left');
      delete left[colour];
      await this.ctx.storage.put('left', left);
    }
    await this.broadcast();
    return new Response(null, { status: 101, webSocket: client });
  }

  async webSocketMessage(ws, text) {
    const { colour } = ws.deserializeAttachment();
    const now = Date.now();
    this.messages = this.messages.filter((t) => now - t < 60_000);
    if (this.messages.length >= MESSAGES_PER_MINUTE) return send(ws, { type: 'error', message: 'Too many messages: wait a minute.' });
    this.messages.push(now);

    let message;
    try {
      message = JSON.parse(text);
    } catch {
      return send(ws, { type: 'error', message: 'not JSON' });
    }
    if (!colour) return send(ws, { type: 'error', message: 'You are watching this game.' });
    try {
      if (await this.act(colour, message) === 'closed') return;
      await this.ctx.storage.setAlarm(Date.now() + KEEP_MS);
    } catch (err) {
      send(ws, { type: 'error', message: err.message });
    }
    await this.broadcast();
  }

  async webSocketClose(ws) {
    closeQuietly(ws);
    const { colour } = ws.deserializeAttachment();
    if (colour && !this.connected(colour, ws)) {
      const left = await this.ctx.storage.get('left');
      if (left) {
        left[colour] = Date.now();
        await this.ctx.storage.put('left', left);
      }
    }
    if (await this.ctx.storage.get('seats')) {
      await this.broadcast();
      if (colour) await this.reportPlayers(ws);
    }
  }

  // Tell the Lobby which players are here, for its count of people online.
  async reportPlayers(except = null) {
    const seats = await this.ctx.storage.get('seats');
    const here = PLAYERS.filter((c) => seats[c] && this.connected(c, except)).map((c) => seats[c].id);
    await lobby(this.env).playing(this.ctx.id.toString(), here);
  }

  async webSocketError(ws) {
    await this.webSocketClose(ws);
  }

  // Whether a socket other than `except` is open for `colour`.
  // (By the attachment, not a tag: a rematch swaps the colours of open sockets.)
  connected(colour, except = null) {
    return this.ctx.getWebSockets().some((ws) =>
      ws !== except && ws.readyState === WebSocket.OPEN && ws.deserializeAttachment().colour === colour);
  }

  async act(colour, message) {
    const s = await this.ctx.storage.get(['seats', 'moves', 'over', 'undo', 'rematch', 'left']);
    const seats = s.get('seats');
    const moves = s.get('moves');
    const over = s.get('over');
    const opponent = other(colour);

    switch (message.type) {
      case 'move': {
        if (over) throw new Error('The game is over.');
        if (!seats[opponent]) throw new Error('Wait for your opponent to join.');
        const square = message.square;
        if (!Number.isInteger(square) || square < 0 || square >= 25) throw new Error('not a square');
        const game = position(moves);
        if (game.toMove !== colour) throw new Error("It is your opponent's move.");
        const next = [...moves, square];
        const after = position(next);          // throws on an illegal move
        await this.ctx.storage.put({ moves: next, undo: null, changedAt: Date.now() });
        if (after.toMove === null) await this.finish({ winner: after.winner, how: after.winner ? 'square' : 'repetition' });
        return;
      }
      case 'cancel':                     // a friend game nobody has joined yet
        if (seats[opponent]) throw new Error('Your opponent has joined: resign instead.');
        await this.close('The game was cancelled.');
        return 'closed';
      case 'resign':
        if (over) throw new Error('The game is over.');
        if (!seats[opponent]) throw new Error('Wait for your opponent to join.');
        return this.finish({ winner: opponent, how: 'resigned' });
      case 'claim': {
        if (over) throw new Error('The game is over.');
        const gone = s.get('left')[opponent];
        if (!seats[opponent] || this.connected(opponent) || !gone || Date.now() - gone < ABANDON_MS) {
          throw new Error('Your opponent has not left.');
        }
        return this.finish({ winner: colour, how: 'abandoned' });
      }
      case 'undo': {
        // Ask to take back your last move (and the reply to it, if there was one).
        if (over) throw new Error('The game is over.');
        if (!moves.some((_, i) => i % 2 === PLAYERS.indexOf(colour))) throw new Error('You have no move to take back.');
        await this.ctx.storage.put('undo', colour);
        return;
      }
      case 'undo-reply': {
        const asker = s.get('undo');
        if (asker !== opponent) throw new Error('There is no take-back to answer.');
        if (message.accept === true) {
          let from = moves.length - 1;
          while (from >= 0 && from % 2 !== PLAYERS.indexOf(asker)) from--;
          await this.ctx.storage.put({ moves: moves.slice(0, from), changedAt: Date.now() });
        }
        await this.ctx.storage.put('undo', null);
        return;
      }
      case 'rematch': {
        if (!over) throw new Error('The game is not over.');
        const rematch = [...new Set([...s.get('rematch'), colour])];
        if (rematch.length < 2) {
          await this.ctx.storage.put('rematch', rematch);
          return;
        }
        // Both want another game: swap colours and start again.
        const swapped = { green: seats.red, red: seats.green };
        for (const ws of this.ctx.getWebSockets()) {
          const { colour: c } = ws.deserializeAttachment();
          if (c) ws.serializeAttachment({ colour: other(c) });
        }
        await this.ctx.storage.put({
          seats: swapped, game: crypto.randomUUID(), moves: [], over: null,
          undo: null, rematch: [], left: {}, ratingChange: null, changedAt: Date.now(),
        });
        await this.ctx.storage.setAlarm(Date.now() + KEEP_MS);
        return;
      }
      default:
        throw new Error(`unknown message "${message.type}"`);
    }
  }

  // The game has ended: store it and update both ratings, each from the
  // other's rating as it is now (another game may have changed it meanwhile).
  async finish(over) {
    const s = await this.ctx.storage.get(['seats', 'moves', 'game']);
    const seats = s.get('seats');
    await this.ctx.storage.put({ over, undo: null, changedAt: Date.now() });
    const now = await Promise.all(PLAYERS.map((c) => ratingOf(this.env, seats[c].id)));
    const greenScore = over.winner === 'green' ? 1 : over.winner === 'red' ? 0 : 0.5;
    const ratingChange = {};
    const statements = [
      this.env.DB.prepare('INSERT OR IGNORE INTO games (id, moves, result, mode, played_at) VALUES (?, ?, ?, ?, ?)')
        .bind(s.get('game'), position(s.get('moves')).moveText, over.winner ?? 'draw', 'online', new Date().toISOString()),
    ];
    PLAYERS.forEach((c, i) => {
      const opponent = now[1 - i];
      const score = c === 'green' ? greenScore : 1 - greenScore;
      const after = glicko2(now[i], [{ rating: opponent.rating, rd: opponent.rd, score }]);
      ratingChange[c] = [shown(now[i]), shown(after)];
      statements.push(this.env.DB.prepare('UPDATE players SET rating = ?, rd = ?, vol = ?, games = games + 1, last_game_at = ? WHERE id = ?')
        .bind(after.rating, after.rd, after.vol, new Date().toISOString(), seats[c].id));
      seats[c] = { ...seats[c], ...after, games: now[i].games + 1 };
    });
    await this.ctx.storage.put({ ratingChange, seats });
    await this.env.DB.batch(statements);
  }

  // Send each socket the game as its owner sees it.
  async broadcast() {
    const s = await this.ctx.storage.get(['seats', 'moves', 'over', 'undo', 'rematch', 'left', 'ratingChange']);
    const seats = s.get('seats');
    if (!seats) return;
    const left = s.get('left');
    const players = Object.fromEntries(PLAYERS.map((c) => [c, seats[c] && {
      ...shown(seats[c]),
      connected: this.connected(c),
      leftAt: left[c] ?? null,
    }]));
    const state = {
      type: 'state',
      moves: s.get('moves'),
      players,
      over: s.get('over'),
      undo: s.get('undo'),
      rematch: s.get('rematch'),
      ratingChange: s.get('ratingChange'),
      abandonMs: ABANDON_MS,
    };
    for (const ws of this.ctx.getWebSockets()) {
      const { colour } = ws.deserializeAttachment();
      send(ws, { ...state, you: colour });
    }
  }
}

// The queue for quick matches: one Durable Object, named "lobby". Each waiting
// player has a socket; when two are matched, the lobby makes a Room for them,
// tells both its code, and closes their sockets.
export class Lobby extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    answerPings(ctx);
  }

  // A socket in the queue (/online/queue), or one that only hears how many
  // are online (/online/presence, from the Online tab's menu).
  async fetch(request) {
    if (request.headers.get('Upgrade') !== 'websocket') return new Response('expected a WebSocket', { status: 426 });
    const url = new URL(request.url);
    const me = await player(this.env, url.searchParams.get('player'), url.searchParams.get('key') ?? '');
    if (!me) return new Response('unknown player, or the wrong key', { status: 403 });
    const queue = url.pathname === '/online/queue';

    const [client, server] = Object.values(new WebSocketPair());
    if (queue) {
      // One place in the queue per player: a second tab replaces the first.
      for (const ws of this.ctx.getWebSockets(`queue:${me.id}`)) ws.close(4000, 'You joined the queue elsewhere.');
      this.ctx.acceptWebSocket(server, ['queue', `queue:${me.id}`]);
      server.serializeAttachment({ ...me, since: Date.now() });
    } else {
      this.ctx.acceptWebSocket(server, ['presence']);
      server.serializeAttachment({ id: me.id });
    }
    if (queue) await this.match();
    await this.announce();
    return new Response(null, { status: 101, webSocket: client });
  }

  async webSocketMessage() {}

  async webSocketClose(ws) {
    closeQuietly(ws);
    await this.tell();
    await this.announce();
  }

  // A Room's connected players (none when it closes), kept for the count.
  async playing(room, ids) {
    const rooms = (await this.ctx.storage.get('rooms')) ?? {};
    if (ids.length) rooms[room] = ids;
    else delete rooms[room];
    await this.ctx.storage.put('rooms', rooms);
    await this.announce(rooms);
  }

  // Tell every socket how many people are online: on the menu, in the queue,
  // or in a game, each player once however many tabs they have.
  async announce(rooms = null) {
    rooms ??= (await this.ctx.storage.get('rooms')) ?? {};
    const open = this.ctx.getWebSockets().filter((ws) => ws.readyState === WebSocket.OPEN);
    const ids = new Set([...open.map((ws) => ws.deserializeAttachment().id), ...Object.values(rooms).flat()]);
    for (const ws of open) send(ws, { type: 'online', count: ids.size });
  }

  async alarm() {
    await this.match();
  }

  waiting() {
    return this.ctx.getWebSockets('queue').filter((ws) => ws.readyState === WebSocket.OPEN)
      .map((ws) => ({ ws, ...ws.deserializeAttachment() }))
      .sort((a, b) => a.since - b.since);
  }

  // Pair the longest-waiting player with the closest rating in reach, until
  // no pair is left; check again soon while anyone waits.
  async match() {
    const now = Date.now();
    // A newcomer's rating is unsure, so their window starts as wide as their deviation.
    const reach = (p) => Math.max(WINDOW, p.rd) + WINDOW_PER_SECOND * (now - p.since) / 1000;
    let waiting = this.waiting();
    while (waiting.length >= 2) {
      let pair = null;
      for (const a of waiting) {
        const candidates = waiting.filter((b) => b !== a && Math.abs(a.rating - b.rating) <= Math.max(reach(a), reach(b)));
        if (candidates.length) {
          candidates.sort((x, y) => Math.abs(a.rating - x.rating) - Math.abs(a.rating - y.rating));
          pair = [a, candidates[0]];
          break;
        }
      }
      if (!pair) break;
      const [a, b] = Math.random() < 0.5 ? pair : [pair[1], pair[0]];
      const code = newCode();
      const seat = ({ id, keyHash, rating, rd, vol, games }) => ({ id, keyHash, rating, rd, vol, games });
      await this.env.ROOMS.get(this.env.ROOMS.idFromName(code)).create({ green: seat(a), red: seat(b) });
      for (const p of [a, b]) {
        send(p.ws, { type: 'matched', game: code });
        p.ws.close(1000, 'matched');
      }
      waiting = waiting.filter((p) => p !== a && p !== b);
    }
    await this.tell(waiting);
    if (waiting.length) await this.ctx.storage.setAlarm(Date.now() + MATCH_EVERY_MS);
  }

  // Tell everyone waiting how many are in the queue.
  async tell(waiting = this.waiting()) {
    for (const p of waiting) send(p.ws, { type: 'queued', waiting: waiting.length, ...shown(p) });
  }
}
