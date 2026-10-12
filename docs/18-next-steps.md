# 18. Next steps

## Open questions

- **Is the game fair?** No: the corner A1 is a forced win for Green at depth 16
  ([opening book](14-opening-book.md)). The simple fix is to ban the corners on the
  first move: no other first move wins at depth 16. The general fix is a swap rule: after
  Green's first move, Red chooses whether to swap colours, so Green wants a first move
  that is neither winning nor losing.
- **What do the other openings come to?** At depth 16, B1 and C3 are draws by
  repetition, C1 and B2 leave Green slightly better, and C2 is level. Deeper searches
  might settle them.
- **The move is worth a lot.** When two copies of the engine at equal strength played
  from short openings, the side to move won over 90% of the games, whether the openings
  were random or chosen by the engine. So engine-versus-engine matches measure who had
  the move, not which engine is stronger ([measuring strength](15-measuring-strength.md)).

## To do

- **Book entries one ply deeper.** The book holds Red's reply to each first move. As
  Green, the engine finds A1 by searching (depth 8 on the page's 400k-node budget). The
  next ply would be Red's best answer to each of Green's second moves.
- **Tests** for the exports the tests don't cover yet: `analyse`, `evaluate`,
  `ai_suggest`, `board_at`, `repetitions`, and the node-budget abort.
- **Late move reductions**, only if more strength is ever wanted.
- **The page:** a variation tree in the move list, and remembering the game across
  reloads.

---

<sub>← [17. Parallel search](17-parallel-search.md) · [index](README.md)</sub>
