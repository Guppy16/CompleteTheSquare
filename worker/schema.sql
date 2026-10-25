-- One row per finished game, sent by the page or played through the connector. No IP or user ID is stored.
CREATE TABLE IF NOT EXISTS games (
  id TEXT PRIMARY KEY,                                   -- random, made by the page per game
  moves TEXT NOT NULL,                                   -- "1. A1 B2  2. C3 ..."
  result TEXT NOT NULL,                                  -- green, red or draw
  mode TEXT NOT NULL,                                    -- board, ai or analysis from the page; claude-green or claude-red
                                                         --   (the colour Claude played) from the connector; online for
                                                         --   a game between two people (online.js)
  played_at TEXT NOT NULL,                               -- when the game ended, from the page
  received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Bug reports sent through the connector's report_bug tool, by Claude for a user.
CREATE TABLE IF NOT EXISTS reports (
  id INTEGER PRIMARY KEY,
  game TEXT,                                             -- the game id, if the report is about a game
  description TEXT NOT NULL,
  received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Players of online games: a random id and a hash of the secret key their browser keeps.
CREATE TABLE IF NOT EXISTS players (
  id TEXT PRIMARY KEY,                                   -- random, made by the browser
  key_hash TEXT NOT NULL,                                -- SHA-256 of the player's key
  rating REAL NOT NULL,                                  -- Glicko-2 rating, from 1500
  rd REAL NOT NULL,                                      -- its deviation, from 350 (unsure)
  vol REAL NOT NULL,                                     -- its volatility, from 0.06
  games INTEGER NOT NULL,                                -- online games played
  last_game_at TEXT,                                     -- the last game's end: the deviation grows after it
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
