// Glicko-2 against the worked example in Glickman's paper, and a few
// properties of single games. Run with: node --test worker/test/
import assert from 'node:assert/strict';
import test from 'node:test';
import { START, aged, glicko2 } from '../ratings.js';

const near = (actual, expected, tolerance) =>
  assert.ok(Math.abs(actual - expected) <= tolerance, `${actual} is not within ${tolerance} of ${expected}`);

test("Glickman's example: 1500 ± 200 after a win and two losses", () => {
  const after = glicko2({ rating: 1500, rd: 200, vol: 0.06 }, [
    { rating: 1400, rd: 30, score: 1 },
    { rating: 1550, rd: 100, score: 0 },
    { rating: 1700, rd: 300, score: 0 },
  ]);
  near(after.rating, 1464.06, 0.01);
  near(after.rd, 151.52, 0.01);
  near(after.vol, 0.05999, 0.00001);
});

test('a win raises a rating, a loss lowers it, by the same amount between equals', () => {
  const win = glicko2(START, [{ rating: 1500, rd: 350, score: 1 }]);
  const loss = glicko2(START, [{ rating: 1500, rd: 350, score: 0 }]);
  assert.ok(win.rating > 1500 && loss.rating < 1500);
  near(win.rating - 1500, 1500 - loss.rating, 1e-6);
  assert.ok(win.rd < START.rd);
});

test('a draw between equals leaves the rating and shrinks the deviation', () => {
  const draw = glicko2({ rating: 1600, rd: 80, vol: 0.06 }, [{ rating: 1600, rd: 80, score: 0.5 }]);
  near(draw.rating, 1600, 1e-6);
  assert.ok(draw.rd < 80);
});

test('beating a much stronger player gains more than beating an equal', () => {
  const me = { rating: 1500, rd: 100, vol: 0.06 };
  const upset = glicko2(me, [{ rating: 1900, rd: 60, score: 1 }]);
  const even = glicko2(me, [{ rating: 1500, rd: 60, score: 1 }]);
  assert.ok(upset.rating - 1500 > even.rating - 1500);
});

test('the deviation never falls below the floor', () => {
  let me = { rating: 1500, rd: 60, vol: 0.06 };
  for (let i = 0; i < 200; i++) me = glicko2(me, [{ rating: 1500, rd: 50, score: 0.5 }]);
  assert.ok(me.rd >= 45);
});

test('the deviation grows with time away, up to the start', () => {
  const settled = { rating: 1700, rd: 60, vol: 0.06 };
  near(aged(settled, 0).rd, 60, 1e-9);
  assert.ok(aged(settled, 30).rd > 60);
  assert.equal(aged(settled, 1e6).rd, START.rd);
  assert.equal(aged(settled, 30).rating, 1700);
});
