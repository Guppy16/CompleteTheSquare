// One game between Claude and a user: a Durable Object, so every request about
// the game reaches the same place. It keeps the moves, checks each one with
// the rules, and holds calls that wait for a move open until the game changes:
// Claude's wait_for_move for the user's click, the board's game_state for
// Claude's reply. The user can also take moves back, put them back and load a
// game from the board. A finished game goes into the games table.

import { DurableObject } from 'cloudflare:workers';
import { other, position } from './rules.js';

// How long a call waits for a move before it returns and is made again.
const WAIT_MS = 45_000;

export class Game extends DurableObject {
  waiting = [];                              // resolves for the calls waiting for a change

  async start(claude) {
    await this.ctx.storage.put({ claude, moves: [], redo: [], id: crypto.randomUUID() });
    return this.view();
  }

  async view() {
    const claude = await this.ctx.storage.get('claude');
    if (!claude) throw new Error('There is no such game: start one with new_game.');
    const redo = (await this.ctx.storage.get('redo')) ?? [];
    return { ...position(await this.ctx.storage.get('moves')), claude, canRedo: redo.length > 0 };
  }

  // Play `square` for Claude or the user, if it is their turn and legal.
  async move(by, square) {
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
    const game = await this.view();
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
    const game = await this.view();
    if (game.toMove === game.claude) throw new Error("Wait for Claude's move first.");
    const redo = (await this.ctx.storage.get('redo')) ?? [];
    if (!redo.length) throw new Error('There is nothing to redo.');
    return this.save([...(await this.ctx.storage.get('moves')), ...redo.at(-1)], redo.slice(0, -1));
  }

  // Replace the game with `squares`; whoever is to move then plays on.
  async load(squares) {
    const game = await this.view();
    if (game.toMove === game.claude) throw new Error("Wait for Claude's move first.");
    return this.save(squares, []);
  }

  // Check and store the moves and the redo stack, and wake every waiting call.
  async save(moves, redo) {
    const claude = await this.ctx.storage.get('claude');
    const game = { ...position(moves), claude, canRedo: redo.length > 0 };
    await this.ctx.storage.put({ moves, redo });
    for (const wake of this.waiting.splice(0)) wake();
    if (game.toMove === null) await this.record(game);
    return game;
  }

  // The game once the user has moved (or it has ended), or as it stands after WAIT_MS.
  async waitForUser() {
    const game = await this.view();
    if (game.toMove !== other(game.claude)) return game;
    await this.change();
    return this.view();
  }

  // The game once it has more than `after` moves (or has ended), or as it
  // stands after WAIT_MS; at once without `after`.
  async watch(after) {
    const game = await this.view();
    if (after === undefined || game.moves.length > after || game.toMove === null) return game;
    await this.change();
    return this.view();
  }

  change() {
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
