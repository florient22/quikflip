# QuikFlip design

QuikFlip is a self-hosted collection of four daily games, running on TrueNAS SCALE. Each game has its own look and feel; the homepage ties them together.

| Game | Mechanic | World |
|---|---|---|
| Letterlock | Guess a 5-letter word in 6 tries | Riso heist: a cheeky padlock opponent, ink layers that snap into register |
| Rarefind | 7 open prompts, rarer answers score more | Skyline: every answer drops a floor onto your skyscraper |
| Atlas Hunt | Pin countries that fit a clue, rarer countries score more | Tiny Planet (three.js): picked countries rise as pillars and launch moons |
| Snippet | Name the song from ever longer clips | Mixtape: a cassette character, tape chewing, a VU meter |

Mockups (claude.ai artifacts): homepage option A, the Letterlock, Rarefind, Atlas Hunt and Snippet pilots.

## Build status

| Part | Status |
|---|---|
| Backend: SQLite, settings, homepage summary | Built |
| Homepage (flip tiles) | Built |
| Letterlock | Built |
| Generator (`quikflip generate --loop`, Claude Code CLI) | Built |
| Rarefind (Skyline) | Built |
| Atlas Hunt, Snippet | Planned |

Dark mode uses pure black backgrounds (OLED); cards and panels sit a hair above black so their edges stay visible.

## Generator

- Runs as a second container from the same image: `quikflip generate --loop`. Each hour it makes sure puzzles exist for today and the next 2 days, in every language in `QUIKFLIP_LANGUAGES`, then judges pending contests. A missed night (NAS off, Claude unavailable) is caught up on the next pass.
- Calls `claude -p --output-format json --tools "" --json-schema …`, authenticated with `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token` (your subscription). Tools are switched off; the CLI validates the answer against the schema, and QuikFlip validates it again.
- Rejected output is sent back once more with the reasons (up to 3 attempts); a day that still fails stays "not ready" and is retried an hour later.

## Rarefind

- 7 prompts a day per language. A prompt's text stays hidden until you start it, and the deadline is kept on the server, so reloading the page does not reset the timer. A wrong answer costs 3 seconds.
- Answers match after folding accents, articles and simple plurals, and with one typo forgiven from 5 letters (two from 9).
- Tiers 1 to 5 score 10, 25, 50, 80 and 100 points and build concrete, brick, glass, gold or neon floors.
- Contests: after the day, a rejected answer can be contested once per prompt. The next generator pass asks Claude for a verdict; an accepted answer is added to the list and the prompt keeps the better score.
- Settings: time per prompt (25 s, 45 s, off), applied from the next prompt you start.

## Architecture

```
TrueNAS SCALE (custom app from compose.yaml)
├── quikflip (serve)     Rust + axum: JSON API + static front end
├── quikflip (generate)  same image, runs the nightly Claude batch   [added with Rarefind]
└── /data (dataset)      quikflip.sqlite3, backed up by TrueNAS snapshots
```

- One Rust binary, `quikflip`, with subcommands. The web server never calls the AI. Puzzles are generated ahead of time, so the AI is never in the request path.
- Front end: plain HTML, CSS and JavaScript in `web/`, one folder per game. No framework or build step. Fonts are self-hosted in `web/fonts/`.
- Database: SQLite in WAL mode (one file, two processes can share it). Postgres would be an extra server for no gain at this size, and DuckDB is an analytics engine with a single writer.
- Schema changes are numbered migrations in `src/db.rs`, tracked with `PRAGMA user_version`. A database from a newer build is refused with a clear error instead of being damaged.

## The "day"

A day runs from midnight to midnight in `QUIKFLIP_TIMEZONE` (default `Europe/Brussels`). Every puzzle is keyed by game, date and variant (language, genre, region…). Settings that change the puzzle (for example Snippet's genre) apply immediately where no AI is involved. For Rarefind and Atlas Hunt they apply from the next day.

## Letterlock

- No AI. Word lists live in `words/` (built by `tools/build_wordlists.py`) and are compiled into the binary.
  - `{lang}-answers.txt`: common 5-letter words that can be the daily word.
  - `{lang}-allowed.txt`: every 5-letter word accepted as a guess.
- Accents are folded (é becomes e) so the keyboard only needs A to Z. Dutch "ij" counts as two letters.
- The daily word is picked at random from answers never used before in that language, then stored, so it never changes during the day and never repeats. When every answer has been used, the one used longest ago comes back.
- The guess is checked on the server (so the answer never reaches the browser before the game ends). Repeated letters are scored correctly. Hard mode means revealed hints must be reused.
- Settings: ink set (Tangerine & Navy, Forest & Coral, Newsprint) and hard mode.

## Avoiding repeats with the AI (from Rarefind on)

The AI never touches the database. The code reads the history, builds the prompt, Claude generates, the code checks every item against the database, duplicates are regenerated, and only valid puzzles are stored.

- Rarefind: the code picks today's categories (least recently used), sends the last ~90 days of prompts as "do not reuse", and rejects near-duplicates by word overlap. Accepted answers are stored per language. Contested answers are judged in the next nightly batch.
- Atlas Hunt: clue types have a cooldown. Data-backed clues (borders, landlocked, flag colours, population) are computed from a country dataset; Claude writes the wording, rarity estimates and fun facts. Facts are stored per country with the date they were shown, so you never see the same fact twice.
- Snippet: songs come from Deezer genre charts and playlists. The track id is unique in the database, and each artist gets a 30-day cooldown. Claude is optional, for themed days.

## Settings

- App-wide: language (EN, FR, NL). The keyboard layout follows the language (QWERTY for English, AZERTY for French and Dutch).
- Per game: stored as JSON with a version number in the `settings` table. Unknown or invalid values fall back to the defaults, so old settings keep working when options change.
- Planned per game: Rarefind timer (25 s, 45 s, off) and categories; Atlas Hunt region and fun facts; Snippet mode (daily mix, random genre, fixed genre) and clip difficulty.

## Word list sources

| List | Source | Licence |
|---|---|---|
| English allowed | ENABLE word list (dolph/dictionary) | Public domain |
| Dutch allowed | OpenTaal word list | BSD-3-Clause or CC BY 3.0 |
| French allowed | an-array-of-french-words | MIT |
| Answer ranking (all languages) | hermitdave/FrequencyWords, OpenSubtitles 2018 | CC BY-SA 4.0 |

Answers are the most frequent words that are also in the dictionary list, minus plurals (English) and the words in `tools/blocklist-{lang}.txt`.
