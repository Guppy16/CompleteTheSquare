// One game between Claude and a user: a Durable Object, so every request about
// the game reaches the same place. It keeps the moves, checks each one with
// the rules, and holds Claude's wait_for_move call open until the user's
// click (user_move) arrives. A finished game goes into the games table.

import { DurableObject } from 'cloudflare:workers';
import { other, position } from './rules.js';

// How long one wait_for_move call waits for the user before Claude asks again.
const WAIT_MS = 45_000;

export class Game extends DurableObject {
  waiting = [];                              // resolves for the open wait_for_move calls

  async start(claude) {
    await this.ctx.storage.put({ claude, moves: [], id: crypto.randomUUID() });
    return this.view();
  }

  async view() {
    const claude = await this.ctx.storage.get('claude');
    if (!claude) throw new Error('There is no such game: start one with new_game.');
    return { ...position(await this.ctx.storage.get('moves')), claude };
  }

  // Play `square` for Claude or the user, if it is their turn and legal.
  async move(by, square) {
    const before = await this.view();
    const side = by === 'claude' ? before.claude : other(before.claude);
    if (before.toMove === null) throw new Error('The game is over.');
    if (before.toMove !== side) {
      throw new Error(by === 'claude' ? "It is the user's move: call wait_for_move." : "It is Claude's move.");
    }
    const moves = [...(await this.ctx.storage.get('moves')), square];
    const after = { ...position(moves), claude: before.claude };
    await this.ctx.storage.put('moves', moves);
    if (by === 'user') for (const wake of this.waiting.splice(0)) wake();
    if (after.toMove === null) await this.record(after);
    return after;
  }

  // The game once the user has moved (or it has ended), or as it stands after WAIT_MS.
  async waitForUser() {
    const game = await this.view();
    if (game.toMove !== other(game.claude)) return game;
    await new Promise((resolve) => {
      this.waiting.push(resolve);
      setTimeout(resolve, WAIT_MS);
    });
    return this.view();
  }

  async record(game) {
    const result = game.winner ?? 'draw';
    await this.env.DB.prepare('INSERT OR IGNORE INTO games (id, moves, result, mode, played_at) VALUES (?, ?, ?, ?, ?)')
      .bind(await this.ctx.storage.get('id'), game.moveText, result, `claude-${game.claude}`, new Date().toISOString())
      .run();
  }
}
