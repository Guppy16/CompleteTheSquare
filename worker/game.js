// One game between Claude and a user: a Durable Object, so every request about
// the game reaches the same place. It keeps the moves, checks each one with
// the rules, and holds calls that wait for a move open until the game changes:
// Claude's wait_for_move for the user's click, the board's game_state for
// Claude's reply. The user can also take moves back, put them back, load a
// game and resign from the board. A finished game goes into the games table.

import { DurableObject } from 'cloudflare:workers';
import { other, position } from './rules.js';

// How long a call waits for a move before it returns and is made again: under
// the few minutes a host gives a tool call.
const WAIT_MS = 4 * 60_000;

// How long Claude waits for the user in all before it ends its turn.
export const IDLE_MS = 20 * 60_000;

// A game is deleted this long after its last change; Claude can start it
// again from its moves (new_game takes them).
const KEEP_MS = 7 * 24 * 60 * 60_000;

// Calls a minute one game takes; a game in play makes a few.
const CALLS_PER_MINUTE = 60;

// A game the user resigned is over, won by Claude.
function withResignation(game, resigned) {
  return resigned ? { ...game, resigned: true, toMove: null, winner: game.claude, winningSquare: [] } : { ...game, resigned: false };
}

export class Game extends DurableObject {
  waiting = [];                              // resolves for the calls waiting for a change
  claudeWaiting = 0;                         // how many of them are Claude's wait_for_move
  lastWaitEnded = 0;                         // when one last returned: Claude calls again in a moment
  calls = [];                                // when the calls of the last minute came

  // Refuse a call beyond CALLS_PER_MINUTE: every call comes from the host's
  // servers, so the limit is per game, not per user.
  limit() {
    const now = Date.now();
    this.calls = this.calls.filter((t) => now - t < 60_000);
    if (this.calls.length >= CALLS_PER_MINUTE) throw new Error('Too many calls to this game: wait a minute.');
    this.calls.push(now);
  }

  // A new game, from `squares` if it resumes one.
  async start(claude, squares) {
    this.limit();
    position(squares);                         // throws before anything is stored
    await this.ctx.storage.put({ claude, id: crypto.randomUUID() });
    return this.save(squares, []);
  }

  async alarm() {
    await this.ctx.storage.deleteAll();
  }

  async view() {
    const claude = await this.ctx.storage.get('claude');
    if (!claude) throw new Error('There is no such game, or it expired a week after its last move: start one with new_game, passing the moves so far to resume it.');
    const redo = (await this.ctx.storage.get('redo')) ?? [];
    const game = { ...position(await this.ctx.storage.get('moves')), claude, canRedo: redo.length > 0 };
    return withResignation(game, await this.ctx.storage.get('resigned'));
  }

  // The user gives up: Claude wins.
  async resign() {
    this.limit();
    const game = await this.view();
    if (game.toMove === null) throw new Error('The game is over.');
    await this.ctx.storage.put({ resigned: true, changedAt: Date.now() });
    const over = await this.view();
    for (const wake of this.waiting.splice(0)) wake();
    await this.record(over);
    return over;
  }

  // Play `square` for Claude or the user, if it is their turn and legal.
  async move(by, square) {
    this.limit();
    const before = await this.view();
    const side = by === 'claude' ? before.claude : other(before.claude);
    if (before.toMove === null) throw new Error('The game is over.');
    if (before.toMove !== side) {
      throw new Error(by === 'claude' ? "It is the user's move: call wait_for_move." : "It is Claude's move.");
    }
    return this.save([...(await this.ctx.storage.get('moves')), square], []);
  }

  // Take back the user's last move and anything after it (Claude's reply), so
  // it is the user's move again.
  async undo() {
    this.limit();
    const game = await this.view();
    if (game.resigned) throw new Error('You resigned this game.');
    if (game.toMove === game.claude) throw new Error("Wait for Claude's move first.");
    const moves = await this.ctx.storage.get('moves');
    const userParity = game.claude === 'green' ? 1 : 0;  // Green plays the even plies
    let from = moves.length - 1;
    while (from >= 0 && from % 2 !== userParity) from--;
    if (from < 0) throw new Error('There is no move of yours to take back.');
    const redo = (await this.ctx.storage.get('redo')) ?? [];
    return this.save(moves.slice(0, from), [...redo, moves.slice(from)]);
  }

  // Put back the moves the last undo took.
  async redo() {
    this.limit();
    const game = await this.view();
    if (game.resigned) throw new Error('You resigned this game.');
    if (game.toMove === game.claude) throw new Error("Wait for Claude's move first.");
    const redo = (await this.ctx.storage.get('redo')) ?? [];
    if (!redo.length) throw new Error('There is nothing to redo.');
    return this.save([...(await this.ctx.storage.get('moves')), ...redo.at(-1)], redo.slice(0, -1));
  }

  // Replace the game with `squares`; whoever is to move then plays on.
  async load(squares) {
    this.limit();
    const game = await this.view();
    if (game.toMove === game.claude) throw new Error("Wait for Claude's move first.");
    await this.ctx.storage.put('resigned', false);
    return this.save(squares, []);
  }

  // Check and store the moves and the redo stack, and wake every waiting call.
  async save(moves, redo) {
    const claude = await this.ctx.storage.get('claude');
    // claudeWaiting tells the board whether a move will reach Claude now, or
    // only once the user sends it a message.
    const claudeWaiting = this.claudeWaiting > 0 || Date.now() - this.lastWaitEnded < 30_000;
    const game = withResignation({ ...position(moves), claude, canRedo: redo.length > 0, claudeWaiting }, false);
    await this.ctx.storage.put({ moves, redo, changedAt: Date.now() });
    await this.ctx.storage.setAlarm(Date.now() + KEEP_MS);
    for (const wake of this.waiting.splice(0)) wake();
    if (game.toMove === null) await this.record(game);
    return game;
  }

  // The game once the user has moved (or it has ended), or as it stands after
  // WAIT_MS, with how long it has gone unchanged.
  async waitForUser() {
    this.limit();
    let game = await this.view();
    if (game.toMove === other(game.claude)) {
      this.claudeWaiting++;
      await this.wait();
      this.claudeWaiting--;
      this.lastWaitEnded = Date.now();
      game = await this.view();
    }
    return { ...game, idleMs: Date.now() - (await this.ctx.storage.get('changedAt')) };
  }

  // The game once it has more than `after` moves (or has ended), or as it
  // stands after WAIT_MS; at once without `after`.
  async watch(after) {
    this.limit();
    const game = await this.view();
    if (after === undefined || game.moves.length > after || game.toMove === null) return game;
    await this.wait();
    return this.view();
  }

  wait() {
    return new Promise((resolve) => {
      this.waiting.push(resolve);
      setTimeout(resolve, WAIT_MS);
    });
  }

  // Only a game's first finish is stored: one taken back and finished again is not.
  async record(game) {
    const result = game.winner ?? 'draw';
    await this.env.DB.prepare('INSERT OR IGNORE INTO games (id, moves, result, mode, played_at) VALUES (?, ?, ?, ?, ?)')
      .bind(await this.ctx.storage.get('id'), game.moveText, result, `claude-${game.claude}`, new Date().toISOString())
      .run();
  }
}
