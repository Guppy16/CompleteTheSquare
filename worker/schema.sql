-- One row per finished game, sent by the page or played through the connector. No IP or user ID is stored.
CREATE TABLE IF NOT EXISTS games (
  id TEXT PRIMARY KEY,                                   -- random, made by the page per game
  moves TEXT NOT NULL,                                   -- "1. A1 B2  2. C3 ..."
  result TEXT NOT NULL,                                  -- green, red or draw
  mode TEXT NOT NULL,                                    -- board, ai or analysis from the page; claude-green or claude-red
                                                         --   (the colour Claude played) from the connector
  played_at TEXT NOT NULL,                               -- when the game ended, from the page
  received_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
