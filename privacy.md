# Privacy policy

Complete the Square is a board game you play on [the website](https://guppy16.github.io/CompleteTheSquare/)
or against Claude, through the Complete the Square connector. This page says what each
of them collects and keeps. Last updated 13 October 2026.

## In short

- No accounts, names, emails or IP addresses are stored. A game cannot be traced back to you.
- What is kept is the games themselves (the moves and the result) and the bug reports you send.
- Nothing is sold or shared with anyone. Games may be published, as anonymous move lists and
  statistics, in the project's analysis of how people and engines play.

## The website

- **Your past games** are saved in your browser (localStorage), so the Analysis tab can list them.
  They stay on your device, and clearing the site's data removes them.
- **Each finished game is sent** to the project's server: a random id made for that game, the
  moves, the result, the mode (over the board, against the engine, or analysis) and when it
  ended. Sending fails silently if you are offline, and the game stays only in your browser.
- **Rate limiting:** the server counts requests per IP address in memory, to limit uploads to a
  few a minute. The count is never stored, and the address is not saved with the game.

## The connector (playing against Claude)

- **While you play,** the server keeps the game: the moves, the colour Claude plays, a random
  id, and when it last changed. A game is deleted a week after its last move.
- **Each finished game** is copied to the same games table as the website's: a random id, the
  moves, the result, which colour Claude played, and when it ended.
- **Bug reports:** if you ask Claude to report a bug, or a tool fails, the server keeps the
  description Claude writes and the game id. Claude is asked to leave out personal details.
- **What the server sees:** only the tools' arguments (a game id, a move, a bug report). Your
  conversation with Claude stays with Anthropic, under
  [Anthropic's privacy policy](https://www.anthropic.com/legal/privacy). The requests come from
  Anthropic's servers, so the connector never sees your IP address.

## Where it is kept, and for how long

- Games and bug reports are kept in a Cloudflare D1 database, and games in play in Cloudflare
  Durable Objects. Cloudflare hosts the server and may keep its own operational logs, under
  [Cloudflare's privacy policy](https://www.cloudflare.com/privacypolicy/). The website is
  hosted on GitHub Pages, under [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
- Finished games and bug reports are kept indefinitely, as they hold nothing personal. Games in
  play are deleted a week after their last move.

## Contact

Questions, or a request to remove a game or a report: open an issue at
[github.com/Guppy16/CompleteTheSquare/issues](https://github.com/Guppy16/CompleteTheSquare/issues).
