// End-to-end test of online games against a deployed Worker (the test copy):
//
//   CTS_URL=https://cts-dev.guppy16.workers.dev node worker/test/online.e2e.mjs
//
// Two scripted players cancel a friend game, then play one (illegal moves, a take-back, a win,
// a rematch, a watcher), then a quick match from the queue, and a game won by
// claiming it when the opponent leaves. Every game is rated. Needs Node 22+ (WebSocket).
import assert from 'node:assert/strict';

const BASE = process.env.CTS_URL;
if (!BASE) throw new Error('set CTS_URL to the Worker, e.g. https://cts-dev.guppy16.workers.dev');
const WS = BASE.replace(/^http/, 'ws');
const sq = (name) => (Number(name[1]) - 1) * 5 + 'ABCDE'.indexOf(name[0]);

function newPlayer() {
  const key = Array.from(crypto.getRandomValues(new Uint8Array(32)), (b) => b.toString(16).padStart(2, '0')).join('');
  return { player: crypto.randomUUID(), key };
}

async function post(path, body) {
  const response = await fetch(BASE + path, { method: 'POST', body: JSON.stringify(body) });
  return { status: response.status, body: response.ok ? await response.json() : await response.text() };
}

// A socket that queues every message, and waits for the next one that matches.
function connect(path, who) {
  const ws = new WebSocket(`${WS}${path}?player=${who.player}&key=${who.key}`);
  const inbox = [];
  const waiters = [];
  ws.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    inbox.push(message);
    for (const w of waiters.splice(0)) w();
  });
  const opened = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve);
    ws.addEventListener('error', () => reject(new Error(`could not connect to ${path}`)));
  });
  return {
    ws,
    opened,
    send: (message) => ws.send(JSON.stringify(message)),
    // The next message for which test(message) holds, skipping the others.
    async next(test = () => true, ms = 10_000) {
      const deadline = Date.now() + ms;
      for (;;) {
        const i = inbox.findIndex(test);
        if (i >= 0) return inbox.splice(0, i + 1).at(-1);
        if (Date.now() > deadline) throw new Error(`no matching message on ${path}; had ${JSON.stringify(inbox.slice(-3))}`);
        await new Promise((resolve) => {
          waiters.push(resolve);
          setTimeout(resolve, 200);
        });
      }
    },
    close: () => ws.close(),
  };
}

const state = (test = () => true) => (m) => m.type === 'state' && test(m);
const step = (text) => console.log(`- ${text}`);

const alice = newPlayer();
const bob = newPlayer();
const carol = newPlayer();

step('players are created at 1500, provisional; a wrong key is refused');
const me = await post('/online/me', alice);
assert.equal(me.status, 200);
assert.deepEqual(me.body, { rating: 1500, provisional: true, games: 0 });
assert.equal((await post('/online/me', { ...alice, key: bob.key })).status, 403);
await post('/online/me', bob);

step('people online: the menu counts each player once, and leaving takes them off');
const pa = connect('/online/presence', alice);
await pa.opened;
const online = (m) => m.type === 'online';
const base = (await pa.next(online)).count;
const pb = connect('/online/presence', bob);
await pb.opened;
await pa.next((m) => online(m) && m.count === base + 1);
pb.close();
await pa.next((m) => online(m) && m.count === base);

step('a friend game nobody has joined can be cancelled, and its link stops working');
const cancelled = (await post('/online/new', { ...alice, colour: 'green' })).body.game;
const x = connect(`/online/game/${cancelled}`, alice);
await x.opened;
await x.next(state());
const closed = new Promise((resolve) => x.ws.addEventListener('close', resolve));
x.send({ type: 'cancel' });
await closed;
await assert.rejects(connect(`/online/game/${cancelled}`, bob).opened);

step('Alice makes a friend game as Green; Bob joins by the link as Red');
const made = await post('/online/new', { ...alice, colour: 'green' });
assert.equal(made.status, 200);
const code = made.body.game;
assert.match(code, /^[a-z2-9]{8}$/);
const a = connect(`/online/game/${code}`, alice);
await a.opened;
let s = await a.next(state());
assert.equal(s.you, 'green');
assert.equal(s.players.red, null);
a.send({ type: 'move', square: sq('C3') });
assert.match((await a.next((m) => m.type === 'error')).message, /opponent to join/);
const b = connect(`/online/game/${code}`, bob);
await b.opened;
s = await b.next(state());
assert.equal(s.you, 'red');
s = await a.next(state((m) => m.players.red?.connected));
assert.equal(s.players.red.rating, 1500);
await pa.next((m) => online(m) && m.count === base + 1);    // Bob counts while in the game
pa.close();
a.send({ type: 'cancel' });
assert.match((await a.next((m) => m.type === 'error')).message, /resign instead/);

step('moves out of turn and onto taken squares are refused');
b.send({ type: 'move', square: sq('E5') });
assert.match((await b.next((m) => m.type === 'error')).message, /opponent's move/);
a.send({ type: 'move', square: sq('A1') });
await b.next(state((m) => m.moves.length === 1));
b.send({ type: 'move', square: sq('A1') });
assert.match((await b.next((m) => m.type === 'error')).message, /illegal/);

step('Bob plays, asks to take it back, Alice accepts');
b.send({ type: 'move', square: sq('E5') });
await a.next(state((m) => m.moves.length === 2));
b.send({ type: 'undo' });
s = await a.next(state((m) => m.undo === 'red'));
a.send({ type: 'undo-reply', accept: true });
s = await b.next(state((m) => m.moves.length === 1 && m.undo === null));

step('a watcher sees the game but cannot move');
const c = connect(`/online/game/${code}`, carol);
await c.opened;
s = await c.next(state());
assert.equal(s.you, null);
c.send({ type: 'move', square: sq('E5') });
assert.match((await c.next((m) => m.type === 'error')).message, /watching/);

step('Green completes a square: A1 B1 A2 B2');
for (const [player, square] of [[b, 'E5'], [a, 'B1'], [b, 'E3'], [a, 'A2'], [b, 'C5'], [a, 'B2']]) {
  player.send({ type: 'move', square: sq(square) });
  await a.next(state((m) => m.moves.at(-1) === sq(square)));
}
s = await b.next(state((m) => m.over));
assert.deepEqual(s.over, { winner: 'green', how: 'square' });
assert.ok(s.ratingChange.green[1].rating > 1500 && s.ratingChange.red[1].rating < 1500);

step('a rematch needs both; then the colours swap');
a.send({ type: 'rematch' });
await b.next(state((m) => m.rematch.includes('green')));
b.send({ type: 'rematch' });
s = await a.next(state((m) => m.over === null && m.moves.length === 0));
assert.equal(s.you, 'red');
s = await b.next(state((m) => m.over === null));
assert.equal(s.you, 'green');

step('Bob (now Green) resigns: Alice wins');
b.send({ type: 'move', square: sq('C3') });
await a.next(state((m) => m.moves.length === 1));
b.send({ type: 'resign' });
s = await a.next(state((m) => m.over));
assert.deepEqual(s.over, { winner: 'red', how: 'resigned' });
for (const x of [a, b, c]) x.close();

const gamesBefore = (await post('/online/me', alice)).body.games;
step('the queue pairs Alice and Bob into a quick match');
const qa = connect('/online/queue', alice);
await qa.opened;
const qb = connect('/online/queue', bob);
await qb.opened;
const ma = await qa.next((m) => m.type === 'matched');
const mb = await qb.next((m) => m.type === 'matched');
assert.equal(ma.game, mb.game);
const ra = connect(`/online/game/${ma.game}`, alice);
const rb = connect(`/online/game/${ma.game}`, bob);
await Promise.all([ra.opened, rb.opened]);
const sa = await ra.next(state());
const sb = await rb.next(state());
assert.notEqual(sa.you, sb.you);
const [green, red] = sa.you === 'green' ? [ra, rb] : [rb, ra];
const redPlayer = sa.you === 'green' ? bob : alice;

step('Green wins the quick match; both ratings move');
for (const [player, square] of [[green, 'A1'], [red, 'E5'], [green, 'B1'], [red, 'E3'], [green, 'A2'], [red, 'C5'], [green, 'B2']]) {
  player.send({ type: 'move', square: sq(square) });
  await green.next(state((m) => m.moves.at(-1) === sq(square)));
}
s = await red.next(state((m) => m.over));
assert.equal(s.over.winner, 'green');
const [gBefore, gAfter] = s.ratingChange.green;
const [rBefore, rAfter] = s.ratingChange.red;
console.log(`  green ${gBefore.rating} -> ${gAfter.rating}, red ${rBefore.rating} -> ${rAfter.rating}`);
assert.ok(gAfter.rating > gBefore.rating && rAfter.rating < rBefore.rating);
assert.equal((await post('/online/me', alice)).body.games, gamesBefore + 1);
assert.equal((await post('/online/me', redPlayer)).body.rating, rAfter.rating);

step('rematch, then Red leaves: Green cannot claim at once, but can after a minute');
green.send({ type: 'rematch' });
red.send({ type: 'rematch' });
await green.next(state((m) => m.over === null && m.moves.length === 0));
const [g2, r2] = [red, green];             // colours swapped
g2.send({ type: 'move', square: sq('C3') });
await r2.next(state((m) => m.moves.length === 1));
r2.close();
await g2.next(state((m) => m.players[m.you === 'green' ? 'red' : 'green'].connected === false));
g2.send({ type: 'claim' });
assert.match((await g2.next((m) => m.type === 'error')).message, /not left/);
console.log('  waiting a minute...');
await new Promise((resolve) => setTimeout(resolve, 61_000));
g2.send({ type: 'claim' });
s = await g2.next(state((m) => m.over));
assert.equal(s.over.how, 'abandoned');
assert.equal(s.over.winner, s.you);
g2.close();

console.log('all passed');
