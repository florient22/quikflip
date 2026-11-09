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
| Atlas Hunt (Tiny Planet) | Built |
| Snippet (Mixtape) | Built |

Dark mode uses pure black backgrounds (OLED); cards and panels sit a hair above black so their edges stay visible.

## Generator

- Runs as a second container from the same image: `quikflip generate --loop`. Each hour it makes sure puzzles exist for today and the next 2 days (Rarefind and Atlas Hunt in every language in `QUIKFLIP_LANGUAGES`, Snippet for every mode and genre), tops up Atlas Hunt's fun facts, then judges pending contests. A missed night (NAS off, Claude or Deezer unavailable) is caught up on the next pass.
- Snippet needs no AI: its songs come from Deezer, so Snippet keeps working even without a Claude token.
- Calls `claude -p --output-format stream-json --verbose --tools "" --json-schema …`, authenticated with `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token` (your subscription). Tools are switched off; the CLI validates the answer against the schema, and QuikFlip validates it again.
- Rejected output is sent back once more with the reasons (up to 3 attempts); a day that still fails stays "not ready" and is retried an hour later.
- Usage limit: the CLI's stream reports the subscription's limit status and reset time. When a call is refused at the limit, the remaining Claude work stops, the reset time (plus 2 minutes) is stored in the database, and the generator sleeps until then instead of a full hour; Snippet carries on as usual. Rarefind, Atlas Hunt and the homepage show that time while a puzzle is missing, and check every minute whether it has arrived. After the reset everything continues on the normal hourly schedule.

## Rarefind

- 7 prompts a day per language. A prompt's text stays hidden until you start it, and the deadline is kept on the server, so reloading the page does not reset the timer. A wrong answer costs 3 seconds.
- Answers match after folding accents, articles and simple plurals, and with one typo forgiven from 5 letters (two from 9).
- Autofill (a setting, on by default): once at least 3 letters and half of an accepted answer are typed, the server returns that answer and the page fills in the rest, selected, so Enter builds it and typing on replaces it. The answer list never goes to the browser.
- The timer shows as the strip under the crate and as a bar shrinking along the answer field.
- After a prompt, the summary lists the best picks (the rarest tier) and, folded away, every accepted answer by tier.
- Tiers 1 to 5 score 10, 25, 50, 80 and 100 points and build concrete, brick, glass, gold or neon floors.
- Contests: after the day, a rejected answer can be contested once per prompt. The next generator pass asks Claude for a verdict; an accepted answer is added to the list and the prompt keeps the better score.
- Settings: time per prompt (25 s, 45 s, off), applied from the next prompt you start; autofill on or off.

## Atlas Hunt

- 5 clues a day per language. Pick one country per clue, by clicking the planet or typing its name (any language, official names and common spellings work). Every country that fits scores; rarer picks score more (the same 10 to 100 points as Rarefind) and rise higher, launching a bigger moon onto a wider orbit. A country can be used once a day. Three misses lose the clue, and the rarest answers are shown.
- Clues are everyday trivia with a surprising answer, in the spirit of Farflung ("German is an official language there": Belgium; "Penguins live there in the wild": South Africa). Every clue needs at least one gem, a fitting country rated tier 5. Overseas territories count for the country they belong to (France borders Brazil through French Guiana).
- Clues come in two kinds. Data clues are computed from the country list (borders, including those through territories; official languages; currencies; landlocked; island nations; number of neighbours), so their answers are complete and correct; Claude only words them and rates rarity. Theme clues (flags, wildlife, rivers and mountains, everyday life, country names, breeds and namesakes, food and drink, sport, history, landmarks, the map) are invented by Claude, which must list every fitting country by code; codes outside the region or list are rejected. Each day has 2 data clues and 3 theme clues, in random order. Regions, sizes and first letters are no longer used: they made dull, school-test clues.
- Repeats: a data clue (for example "borders Germany") rests 60 days; themes rotate least-recently-used first; invented clues are checked against the last 90 days by wording.
- Notes: with every clue, Claude writes one short sentence per fitting country on why it fits (for "German is an official language there", Belgium: the German-speaking community around Eupen). Finding the country shows that note, so the fact always belongs to the question.
- Fun facts: a fallback pool per country and language, for answers without a note (a note that is empty or too long is dropped, and puzzles from before 0.4.0 have none). You get a fact you have never seen, and it is marked as shown so it never comes back. The generator keeps at least 2 unseen facts only for countries that can be found in the coming days without a note.
- Settings: region (whole world, Europe, Africa, Americas, Asia, Oceania) and fun facts on or off. A new region applies from tomorrow: later puzzles made for the old region and not played yet are made again.
- The planet is three.js; countries are drawn from Natural Earth 1:50m shapes. Very small countries (and Tuvalu, which has no shape at that scale) get a hexagonal column instead of a raised outline. The page needs WebGL.

## Snippet

- One song a day per mode and genre; six chances. Each skip or wrong guess unlocks a longer clip. A guess is right when the title matches (versions, "feat." parts and a typo or two are ignored); if you add " — artist" the artist must match too. Suggestions come from a local catalog of chart songs, so typing never calls Deezer.
- The server cuts the MP3 at whole frames and sends only the seconds you have unlocked; the full 30-second preview and the album cover are sent only after the song is finished.
- Songs come from two chart sources. Worldwide (the default): Deezer's "Top Worldwide" playlist plus its genre editors' international playlists (Deezer's chart endpoints follow the caller's country, so they can't give worldwide genre charts). Local: Deezer's charts for the country the generator runs in, so Belgian charts on a Belgian NAS. Pools are widened with genre artists' top tracks when they run dry; a retired playlist is logged and skipped. A track is never used twice and an artist rests 30 days. The generator stores the preview and cover in the database and deletes the audio once the day is over (song details stay, for the history).
- Settings, applied right away (every chart source, mode and genre has its own song each day): charts (worldwide or local), mode (daily mix, a random genre each day shown as a hint, or one genre), genre (pop, hip-hop, rock, dance, R&B, alternative, electro, metal, soul and funk, latin, French chanson, Dutch-language), and clip length (easy 2 to 24 s, normal 1 to 16 s, hard 0.5 to 15 s).

## Architecture

```
TrueNAS SCALE (custom app from compose.yaml)
├── quikflip (serve)     Rust + axum: JSON API + static front end
├── quikflip (generate)  same image, runs the nightly Claude batch   [added with Rarefind]
└── /data (volume)       quikflip.sqlite3 in the quikflip-data Docker volume
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
- Atlas Hunt: data clues rest 60 days and themes rotate; data-backed answers are computed from the country list; Claude writes the wording, rarity estimates and fun facts. Facts are stored per country with the date they were shown, so you never see the same fact twice.
- Snippet (no AI): the track id is unique in the database, and each artist gets a 30-day cooldown.

## Settings

- App-wide: language (EN, FR, NL). The keyboard layout follows the language (QWERTY for English, AZERTY for French and Dutch).
- Per game: stored as JSON with a version number in the `settings` table. Unknown or invalid values fall back to the defaults, so old settings keep working when options change.
- Per game: Letterlock ink set and hard mode; Rarefind timer (25 s, 45 s, off); Atlas Hunt region and fun facts; Snippet mode (daily mix, random genre, one genre), genre and clip length.

## Word list sources

| List | Source | Licence |
|---|---|---|
| English allowed | ENABLE word list (dolph/dictionary) | Public domain |
| Dutch allowed | OpenTaal word list | BSD-3-Clause or CC BY 3.0 |
| French allowed | an-array-of-french-words | MIT |
| Answer ranking (all languages) | hermitdave/FrequencyWords, OpenSubtitles 2018 | CC BY-SA 4.0 |

Answers are the most frequent words that are also in the dictionary list, minus plurals (English) and the words in `tools/blocklist-{lang}.txt`.

## Other data and libraries

| What | Source | Licence |
|---|---|---|
| Country names (EN/FR/NL), borders, languages, currencies, area (`web/games/atlas/data/countries.json`, built by `tools/build_countries.py`) | mledoze/countries | ODbL 1.0 |
| Country shapes (`web/games/atlas/data/world.json`) | world-atlas 2.0.2 (Natural Earth 1:50m) | ISC; Natural Earth is public domain |
| three.js 0.160, d3 7.9, topojson-client 3.1 (`web/vendor/`, fetched by `tools/fetch_vendor.py`) | npm | MIT, ISC, ISC |
| Song previews and covers | Deezer API (free, no key), fetched by the generator | Deezer's terms; 30-second previews for personal use |
