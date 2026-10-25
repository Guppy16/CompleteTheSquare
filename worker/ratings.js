// Glicko-2 ratings for online games (online.js), in a module of their own so
// they can be tested outside Cloudflare: node --test worker/test/
//
// Ratings are Glicko-2 (Glickman, "Example of the Glicko-2 system", 2013): a
// rating, a deviation (how sure the rating is) and a volatility (how erratic
// the player's results are). A new player starts unsure, at 1500 ± 350, and
// settles within a few games; the deviation grows again while they do not
// play. Each rated game is its own rating period, as on lichess.
export const START = { rating: 1500, rd: 350, vol: 0.06 };
const TAU = 0.5;                      // how fast volatility may change
const MIN_RD = 45;                    // never quite certain
const SCALE = 173.7178;               // Glicko-2's internal scale: 400 / ln 10
const RD_GROWTH_DAYS = 1;             // one rating period of inactivity per day
export const PROVISIONAL_RD = 110;    // shown with a "?" above this

const g = (phi) => 1 / Math.sqrt(1 + (3 * phi * phi) / (Math.PI * Math.PI));

// The deviation after `days` without a rated game.
export function aged({ rating, rd, vol }, days) {
  const phi = rd / SCALE;
  const periods = Math.max(0, days / RD_GROWTH_DAYS);
  return { rating, rd: Math.min(START.rd, SCALE * Math.sqrt(phi * phi + vol * vol * periods)), vol };
}

// A player's new { rating, rd, vol } after `results`: [{ rating, rd, score }],
// score 1 for a win, 0.5 for a draw, 0 for a loss.
export function glicko2(me, results) {
  const mu = (me.rating - START.rating) / SCALE;
  const phi = me.rd / SCALE;
  const games = results.map((r) => {
    const gj = g(r.rd / SCALE);
    const e = 1 / (1 + Math.exp(-gj * (mu - (r.rating - START.rating) / SCALE)));
    return { gj, e, score: r.score };
  });
  const v = 1 / games.reduce((sum, { gj, e }) => sum + gj * gj * e * (1 - e), 0);
  const delta = v * games.reduce((sum, { gj, e, score }) => sum + gj * (score - e), 0);

  // The new volatility: the root of f, by the Illinois algorithm.
  const a = Math.log(me.vol * me.vol);
  const f = (x) => {
    const ex = Math.exp(x);
    const d = phi * phi + v + ex;
    return (ex * (delta * delta - phi * phi - v - ex)) / (2 * d * d) - (x - a) / (TAU * TAU);
  };
  let A = a;
  let B;
  if (delta * delta > phi * phi + v) {
    B = Math.log(delta * delta - phi * phi - v);
  } else {
    let k = 1;
    while (f(a - k * TAU) < 0) k++;
    B = a - k * TAU;
  }
  let fA = f(A);
  let fB = f(B);
  while (Math.abs(B - A) > 1e-6) {
    const C = A + ((A - B) * fA) / (fB - fA);
    const fC = f(C);
    if (fC * fB <= 0) {
      A = B;
      fA = fB;
    } else {
      fA /= 2;
    }
    B = C;
    fB = fC;
  }
  const vol = Math.exp(A / 2);

  const phiStar = Math.sqrt(phi * phi + vol * vol);
  const phiNew = 1 / Math.sqrt(1 / (phiStar * phiStar) + 1 / v);
  const muNew = mu + phiNew * phiNew * games.reduce((sum, { gj, e, score }) => sum + gj * (score - e), 0);
  return { rating: START.rating + SCALE * muNew, rd: Math.max(MIN_RD, SCALE * phiNew), vol };
}
