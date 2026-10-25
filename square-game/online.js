// Online games against another person, through the Worker (worker/online.js).
//
// The server holds the game: this page sends moves and requests over a
// WebSocket and redraws from the state the server sends back. A move is shown
// at once and confirmed by the next state (a refused one is undone by it).
// Every game is rated. A quick match comes from the queue; a friend game from
// a link (?game=<code>).
//
// The player is a random id and key kept in localStorage (key cts:player), so
// a rating follows this browser. Without storage, each page load is a new player.
// Loaded after script.js, whose functions and state it uses. The panel's
// elements are in index.html and only change class and text, so their
// transitions run.

const ONLINE_URL = ['localhost', '127.0.0.1'].includes(location.hostname)
  ? 'https://cts-dev.guppy16.workers.dev'        // the test copy (worker/deploy.py)
  : 'https://cts.guppy16.workers.dev';
const ONLINE_WS = ONLINE_URL.replace(/^http/, 'ws');
const PLAYER_KEY = 'cts:player';
const HISTORY_LENGTH = 50;             // games in the rating sparkline
const PING_MS = 30_000;
const RECONNECT_MS = [500, 1000, 2000, 4000, 8000];
const SLIDE_COMMIT = 0.85;             // how far the quick-match knob must go
const HOLD_MS = 1000;                  // how long to hold Resign
const NOTICE_MS = 4000;                // how long an error shows in the snackbar
const COPIED_MS = 1400;                // how long the link says Copied

const onlineEl = document.getElementById('online');
const menuEl = document.getElementById('online-menu');
const myRatingEl = menuEl.querySelector('.rating');
const historyEl = menuEl.querySelector('.history');
const slideEl = document.getElementById('quick-match');
const knobEl = slideEl.querySelector('.slide-knob');
const slideLabelEl = slideEl.querySelector('.slide-label');
const friendButton = document.getElementById('play-friend');
const inviteEl = document.getElementById('online-invite');
const linkButton = inviteEl.querySelector('.link');
const linkLabelEl = linkButton.querySelector('.label');
const playersEl = document.getElementById('players');
const playerEls = Array.from(playersEl.querySelectorAll('.player'));
const resignButton = document.getElementById('resign');
const quickAgainButton = document.getElementById('quick-again');
const snackbarEl = document.getElementById('snackbar');

// view: 'menu', 'queue' or 'game'. state: the last state from the server.
// socket: the open WebSocket, to the queue or to a game.
const online = {
  view: 'menu', code: null, state: null, socket: null, me: null, waiting: 0,
  notice: null, retries: 0, pinger: null,
};

function onlinePlayer() {
  try {
    const saved = localStorage.getItem(PLAYER_KEY);
    if (saved) return JSON.parse(saved);
  } catch (err) {
    console.warn('Online rating is not kept: storage is blocked.', err);
  }
  const key = Array.from(crypto.getRandomValues(new Uint8Array(32)), b => b.toString(16).padStart(2, '0')).join('');
  const made = { player: crypto.randomUUID(), key };
  try {
    localStorage.setItem(PLAYER_KEY, JSON.stringify(made));
  } catch {
    // as above: a new player per page load
  }
  return made;
}
const identity = onlinePlayer();

async function onlinePost(path, body) {
  const response = await fetch(ONLINE_URL + path, { method: 'POST', body: JSON.stringify({ ...identity, ...body }) });
  if (!response.ok) throw new Error(`${path}: ${response.status} ${await response.text()}`);
  return response.json();
}

const opposite = colour => (colour === 'green' ? 'red' : 'green');

// --- Hooks for script.js -------------------------------------------------------

// The player may not move: no game, the opponent's move, or the game is over.
function onlineWaiting() {
  if (mode !== 'online') return false;
  const s = online.state;
  if (online.view !== 'game' || !s?.you || s.over) return true;
  if (!s.players[opposite(s.you)]) return true;                 // the friend has not joined
  return ai.current_player() !== (s.you === 'green' ? 0 : 1);
}

// A result the engine cannot see: a resignation or an abandoned game.
function onlineOver() {
  return mode === 'online' && online.view === 'game' && online.state?.over != null;
}

function onlineWinner() {
  const winner = online.state.over.winner;
  return winner === 'green' ? 0 : winner === 'red' ? 1 : -1;
}

// The status line's note on an online game: how it ended, or that you watch.
function onlineStatusNote() {
  const s = online.state;
  if (online.view !== 'game' || !s) return '';
  if (s.over?.how === 'resigned') return ` <small>(${getColorName(1 - onlineWinner())} resigned)</small>`;
  if (s.over?.how === 'abandoned') return ` <small>(${getColorName(1 - onlineWinner())} left)</small>`;
  return s.you ? '' : ' <small>(watching)</small>';
}

// Undo asks to take back your last move: once you have one, the opponent is
// there, and you have not asked already.
function onlineCanUndo() {
  const s = online.state;
  if (online.view !== 'game' || !s?.you || s.over || s.undo === s.you || !s.players[opposite(s.you)]) return false;
  const parity = s.you === 'green' ? 0 : 1;
  return s.moves.some((_, i) => i % 2 === parity);
}

function onlineCanRematch() {
  const s = online.state;
  return online.view === 'game' && Boolean(s?.you && s.over) && !s.rematch.includes(s.you);
}

function onlineClick(row, col) {
  if (onlineWaiting()) return;
  if (!ai.play(row, col)) return;      // occupied: nothing to send
  logMoves();
  render();
  onlineSend({ type: 'move', square: row * BOARD_SIZE + col });
}

function onlineUndo() {
  onlineSend({ type: 'undo' });
}

function onlineRematch() {
  onlineSend({ type: 'rematch' });
}

// Entering the tab: the game in play, or (after a finished one) the menu.
function onlineEnter() {
  if (online.view === 'game' && online.state?.over) leaveGame();
  else if (online.view === 'game' && online.state) onlineSync(online.state.moves);
  else if (online.view === 'menu') ai.reset();
  if (online.view === 'menu') refreshMyRating();
}

// --- Connections ---------------------------------------------------------------

function onlineSend(message) {
  if (online.socket?.readyState === WebSocket.OPEN) online.socket.send(JSON.stringify(message));
  else notify('Not connected: reconnecting…');
}

function closeSocket() {
  clearInterval(online.pinger);
  if (online.socket) {
    online.socket.onclose = null;
    online.socket.close();
  }
  online.socket = null;
}

// onClose(event, opened) runs when the socket closes by itself.
function openSocket(path, onMessage, onClose) {
  closeSocket();
  const ws = new WebSocket(`${ONLINE_WS}${path}?player=${identity.player}&key=${identity.key}`);
  online.socket = ws;
  let opened = false;
  ws.onopen = () => {
    opened = true;
    online.retries = 0;
    online.pinger = setInterval(() => ws.readyState === WebSocket.OPEN && ws.send('ping'), PING_MS);
  };
  ws.onmessage = ({ data }) => {
    if (data !== 'pong') onMessage(JSON.parse(data));
  };
  ws.onclose = event => {
    clearInterval(online.pinger);
    if (online.socket === ws) online.socket = null;
    onClose(event, opened);
  };
}

function joinGame(code) {
  online.view = 'game';
  online.code = code;
  online.state = null;
  history.replaceState(null, '', `?game=${code}`);
  ai.reset();
  render();
  openSocket(`/online/game/${code}`, onGameMessage, (event, opened) => {
    if (online.view !== 'game' || online.code !== code) return;
    // Never connected: no such game (or no network). Once connected: reconnect.
    if (!opened && online.state === null) {
      leaveGame();
      notify('Could not open that game: it does not exist, it has expired, or you are offline.');
      return;
    }
    const wait = RECONNECT_MS[Math.min(online.retries++, RECONNECT_MS.length - 1)];
    render();
    setTimeout(() => {
      if (online.view === 'game' && online.code === code && !online.socket) joinGame(code);
    }, wait);
  });
}

function onGameMessage(message) {
  if (message.type === 'error') return notify(message.message);
  if (message.type !== 'state') return;
  online.state = message;
  if (mode === 'online') {
    onlineSync(message.moves);
    render();
  }
}

// Bring the engine to the server's moves: play on when they extend the
// board's (so the opponent's move animates), otherwise replay from the start.
function onlineSync(moves) {
  const local = Array.from({ length: ai.move_count() }, (_, i) => ai.move_at(i));
  const extend = local.length <= moves.length && local.every((m, i) => m === moves[i]);
  if (!extend) ai.reset();
  for (const square of moves.slice(extend ? local.length : 0)) {
    if (!ai.play(Math.floor(square / BOARD_SIZE), square % BOARD_SIZE)) {
      throw new Error(`the server sent an illegal move, ${squareName(square)}`);
    }
  }
}

function findOpponent() {
  closeSocket();
  online.view = 'queue';
  online.code = null;
  online.state = null;
  online.waiting = 0;
  history.replaceState(null, '', location.pathname);
  ai.reset();
  render();
  openSocket('/online/queue', message => {
    if (message.type === 'queued') {
      online.waiting = message.waiting;
      online.me = { ...online.me, rating: message.rating, provisional: message.provisional };
      renderOnline();
    } else if (message.type === 'matched') {
      joinGame(message.game);
    }
  }, (event, opened) => {
    if (online.view !== 'queue') return;
    if (event.code === 4000) stopSearch('You joined the queue in another tab.');
    else if (!opened) stopSearch('Could not reach the server: are you offline?');
    else setTimeout(() => online.view === 'queue' && findOpponent(), 1000);
  });
}

function stopSearch(message) {
  closeSocket();
  online.view = 'menu';
  render();
  if (message) notify(message);
}

async function playFriend() {
  let code;
  try {
    code = (await onlinePost('/online/new', { colour: 'random' })).game;
  } catch (err) {
    return notify(`Could not start a game: ${err.message}`);
  }
  joinGame(code);
  copyLink();                          // still within the click, so the browser allows it
}

const gameLink = () => `${location.origin}${location.pathname}?game=${online.code}`;

// Copy the invite link: the copy icon turns into a tick for a moment. A
// browser may refuse (no click recently, or no permission); then the link
// still says Copy, for a tap.
let copiedTimer = null;
async function copyLink() {
  try {
    await navigator.clipboard.writeText(gameLink());
  } catch (err) {
    console.warn('Could not copy the link', err);
    return;
  }
  linkButton.classList.add('copied');
  linkLabelEl.textContent = 'Copied';
  clearTimeout(copiedTimer);
  copiedTimer = setTimeout(() => {
    linkButton.classList.remove('copied');
    linkLabelEl.textContent = 'Copy';
  }, COPIED_MS);
}

// Back to the menu (a finished game, a friend who never came, a bad link).
function leaveGame() {
  closeSocket();
  online.view = 'menu';
  online.code = null;
  online.state = null;
  history.replaceState(null, '', location.pathname);
  ai.reset();
  render();
  refreshMyRating();
}

async function refreshMyRating() {
  try {
    online.me = await onlinePost('/online/me', {});
    renderOnline();
  } catch (err) {
    console.warn('Could not fetch the online rating', err);
  }
}

// A link to a game opens it in the Online tab.
function onlineFromUrl() {
  const code = new URLSearchParams(location.search).get('game');
  if (!code) return;
  setMode('online');
  joinGame(code.toLowerCase());
}

// --- Rating history ----------------------------------------------------------
// Each online game in the past games (script.js) keeps your rating going into
// it; the sparkline is those, then the rating now.

// Your rating before the game just finished, for its past-games record.
function onlineRatingBefore() {
  const s = online.state;
  return s?.you ? s.ratingChange?.[s.you][0].rating : undefined;
}

// A sparkline of the ratings, with a dot on the latest; hidden before two.
function drawHistory() {
  const before = readPastGames().filter(game => game.rating).slice(0, HISTORY_LENGTH).reverse();
  const ratings = [...before.map(game => game.rating), online.me.rating];
  historyEl.toggleAttribute('hidden', ratings.length < 2);   // an svg has no .hidden
  if (ratings.length < 2) return;
  const low = Math.min(...ratings);
  const span = Math.max(Math.max(...ratings) - low, 50);   // a few points do not fill the height
  const points = ratings.map((r, i) => [i * 120 / (ratings.length - 1), 21 - (r - low) * 18 / span]);
  historyEl.querySelector('polyline').setAttribute('points', points.join(' '));
  const [x, y] = points.at(-1);
  historyEl.querySelector('circle').setAttribute('cx', x);
  historyEl.querySelector('circle').setAttribute('cy', y);
}

// --- Pieces of the panel ------------------------------------------------------

const ratingText = ({ rating, provisional }) => `${rating}${provisional ? ', provisional' : ''}`;

// An odometer: each digit is a reel of 0-9 that rolls to its place. Rebuilt
// only when the number of digits changes.
function setOdometer(el, { rating, provisional }) {
  const digits = String(rating).split('');
  if (el.querySelectorAll('.reel').length !== digits.length) {
    el.innerHTML = digits.map(() => '<span class="digit"><span class="reel">0 1 2 3 4 5 6 7 8 9</span></span>').join('') +
                   '<span class="mark" tabindex="0" data-tip="Provisional for your first ~10 games">?</span>';
  }
  el.querySelectorAll('.reel').forEach((reel, i) => reel.style.setProperty('--d', digits[i]));
  el.querySelector('.mark').hidden = !provisional;
  el.setAttribute('role', 'img');
  el.setAttribute('aria-label', ratingText({ rating, provisional }));
}

// The snackbar under the controls: one message, maybe with buttons or a
// countdown ring. Messages come from the state (requests, a player who left)
// and from errors, which show for a few seconds.
let snackKey = '';
function setSnack(message) {
  const key = message ? JSON.stringify([message.text, message.actions?.map(a => a.label), message.until]) : '';
  if (key === snackKey) return;
  snackKey = key;
  snackbarEl.classList.toggle('shown', Boolean(message));
  if (!message) return;
  snackbarEl.replaceChildren();
  if (message.until) {
    const ring = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    ring.classList.add('ring');
    ring.setAttribute('viewBox', '0 0 20 20');
    ring.innerHTML = '<circle cx="10" cy="10" r="8" pathLength="100"/>';
    ring.style.setProperty('--left', `${Math.max(0, message.until - Date.now())}ms`);
    snackbarEl.append(ring);
  }
  const text = document.createElement('span');
  text.textContent = message.text;
  snackbarEl.append(text);
  for (const { label, onClick } of message.actions ?? []) {
    const button = document.createElement('button');
    button.textContent = label;
    button.addEventListener('click', onClick);
    snackbarEl.append(button);
  }
}

let noticeTimer = null;
function notify(text) {
  online.notice = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    online.notice = null;
    renderOnline();
  }, NOTICE_MS);
  renderOnline();
}

// What the snackbar should say now, most urgent first.
function snackMessage() {
  if (online.notice) return { text: online.notice };
  const s = online.state;
  if (online.view !== 'game') return null;
  if (!s || !online.socket) return { text: s ? 'Reconnecting…' : 'Connecting…' };
  if (!s.you) return null;
  const them = s.players[opposite(s.you)];
  if (!s.over && s.undo === opposite(s.you)) {
    return { text: 'Your opponent wants to take back their move', actions: [
      { label: 'Accept', onClick: () => onlineSend({ type: 'undo-reply', accept: true }) },
      { label: 'Decline', onClick: () => onlineSend({ type: 'undo-reply', accept: false }) },
    ] };
  }
  if (!s.over && them && !them.connected && them.leftAt) {
    const until = them.leftAt + s.abandonMs;
    if (Date.now() >= until) {
      return { text: 'Your opponent left', actions: [{ label: 'Claim the win', onClick: () => onlineSend({ type: 'claim' }) }] };
    }
    setTimeout(renderOnline, until - Date.now() + 50);
    return { text: 'Your opponent left: you can claim the win if they do not come back', until };
  }
  if (!s.over && s.undo === s.you) return { text: 'Asked to take back your move' };
  if (s.over && s.rematch.includes(opposite(s.you)) && !s.rematch.includes(s.you)) return { text: 'Your opponent wants a rematch' };
  if (s.over && s.rematch.includes(s.you)) return { text: 'Rematch offered' };
  return null;
}

// --- The panel ---------------------------------------------------------------

// Redraws the Online panel, the snackbar and the online-only buttons; render() calls it.
function renderOnline() {
  const isOnline = mode === 'online';
  onlineEl.hidden = !isOnline;
  // No board until there is a game: on the menu, in the queue.
  document.body.classList.toggle('no-game', isOnline && online.view !== 'game');
  const s = online.state;
  const inGame = isOnline && online.view === 'game';
  const playing = inGame && Boolean(s?.you) && !s.over && Boolean(s.players[opposite(s.you)]);
  resignButton.hidden = !playing;
  quickAgainButton.hidden = !(inGame && s?.over && s.you);
  setSnack(isOnline ? snackMessage() : null);
  if (!isOnline) return;

  const waitingForFriend = inGame && s?.you && !s.players[opposite(s.you)];
  menuEl.hidden = inGame;
  inviteEl.hidden = !waitingForFriend;
  playersEl.hidden = !inGame || !s || waitingForFriend;

  if (!inGame) {
    const searching = online.view === 'queue';
    slideEl.classList.toggle('searching', searching);
    knobEl.setAttribute('aria-label', searching ? 'Stop searching' : 'Quick match');
    if (online.me) {
      setOdometer(myRatingEl, online.me);
      myRatingEl.parentElement.hidden = false;
      drawHistory();
    }
    slideLabelEl.textContent = !searching ? 'slide to play'
      : online.waiting > 1 ? `finding someone: ${online.waiting} waiting` : 'finding someone';
    friendButton.disabled = searching;
    return;
  }
  if (waitingForFriend) {
    linkButton.querySelector('.url').textContent = gameLink().replace(/^https?:\/\//, '');
    return;
  }
  if (!s) return;

  // Two chips: yours first (Green first for a watcher); the pill sits behind
  // the side to move.
  const order = s.you ? [s.you, opposite(s.you)] : ['green', 'red'];
  order.forEach((colour, i) => {
    const el = playerEls[i];
    const player = s.players[colour];
    el.dataset.colour = colour;
    el.querySelector('.who').textContent = s.you ? (i === 0 ? 'You' : 'Opponent') : getColorName(colour === 'green' ? 0 : 1);
    const change = s.ratingChange?.[colour];
    setOdometer(el.querySelector('.rating'), change ? change[1] : player);
    const delta = change ? change[1].rating - change[0].rating : null;
    const deltaEl = el.querySelector('.delta');
    deltaEl.textContent = delta === null ? '' : `${delta >= 0 ? '+' : '−'}${Math.abs(delta)}`;
    deltaEl.classList.toggle('up', delta > 0);
    deltaEl.classList.toggle('down', delta < 0);
    el.classList.toggle('away', !player.connected && !s.over);
  });
  const toMove = gameOver() ? null : ai.current_player() === 0 ? 'green' : 'red';
  playersEl.classList.toggle('no-turn', toMove === null);
  playersEl.classList.toggle('over', Boolean(s.over));
  playersEl.classList.toggle('second', toMove === order[1]);
}

// --- Gestures -------------------------------------------------------------------

// Slide to start a quick match: drag the knob to the end of the track. A tap
// does nothing (so it cannot start by accident); Enter or Space does. While
// searching, the knob stops the search.
(function slideToMatch() {
  let start = null;
  let wasSearching = false;            // the click after a slide must not stop the search it started
  const travel = () => slideEl.clientWidth - knobEl.offsetWidth - 8;
  const place = x => {
    slideEl.style.setProperty('--x', `${x}px`);
    slideEl.style.setProperty('--progress', x / travel());
  };
  knobEl.addEventListener('pointerdown', event => {
    wasSearching = online.view === 'queue';
    if (online.view !== 'menu') return;
    start = event.clientX;
    knobEl.setPointerCapture(event.pointerId);
    slideEl.classList.add('dragging');
  });
  knobEl.addEventListener('pointermove', event => {
    if (start !== null) place(Math.min(travel(), Math.max(0, event.clientX - start)));
  });
  const release = event => {
    if (start === null) return;
    const done = event.type === 'pointerup' && event.clientX - start >= travel() * SLIDE_COMMIT;
    start = null;
    slideEl.classList.remove('dragging');
    place(0);                          // springs back, or under the searching state's end position
    if (done) findOpponent();
  };
  knobEl.addEventListener('pointerup', release);
  knobEl.addEventListener('pointercancel', release);
  knobEl.addEventListener('click', event => {
    const keyboard = event.detail === 0;
    if (online.view === 'queue' && (keyboard || wasSearching)) stopSearch();
    else if (online.view === 'menu' && keyboard) findOpponent();
  });
})();

// Hold Resign to resign: the button fills while held; letting go early cancels.
(function holdToResign() {
  let timer = null;
  const begin = () => {
    if (timer) return;
    resignButton.classList.add('holding');
    timer = setTimeout(() => {
      end();
      onlineSend({ type: 'resign' });
    }, HOLD_MS);
  };
  const end = () => {
    clearTimeout(timer);
    timer = null;
    resignButton.classList.remove('holding');
  };
  resignButton.addEventListener('pointerdown', begin);
  for (const type of ['pointerup', 'pointerleave', 'pointercancel', 'blur']) resignButton.addEventListener(type, end);
  resignButton.addEventListener('keydown', event => {
    if ((event.key === 'Enter' || event.key === ' ') && !event.repeat) {
      event.preventDefault();
      begin();
    }
  });
  resignButton.addEventListener('keyup', end);
})();

friendButton.addEventListener('click', playFriend);
quickAgainButton.addEventListener('click', findOpponent);
linkButton.addEventListener('click', copyLink);
