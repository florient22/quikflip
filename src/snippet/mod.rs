//! Snippet: name the song from ever longer clips. Every skip or wrong guess unlocks a longer clip; six chances.
//! Songs and their audio are picked ahead of time by the generator (from Deezer), one per mode and genre.

pub mod deezer;
pub mod generate;
pub mod mp3;

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::day::{self, format_day};
use crate::error::AppError;
use crate::settings::{Language, SnippetGenre, SnippetMode, SnippetSettings};
use crate::text;

pub const MAX_GUESSES: usize = 6;
/// The whole preview, once the song is solved or all guesses are used.
pub const FULL_SECONDS: f64 = 30.0;
pub const MIX: &str = "mix";
pub const RANDOM: &str = "random";
const MAX_GUESS_CHARS: usize = 200;
const SUGGESTION_LIMIT: usize = 8;
const SUGGESTION_SCAN: i64 = 300;
/// What a suggestion puts between title and artist; a guess typed this way is split back apart.
pub const TITLE_ARTIST_SEPARATOR: &str = " — ";
const SKIP: &str = "skip";
const WRONG: &str = "wrong";
const RIGHT: &str = "right";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Waiting,
    New,
    Progress,
    Solved,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuessView {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub date: String,
    pub variant: String,
    pub mode: SnippetMode,
    /// Today's genre, shown as a hint in the random and genre modes.
    pub genre: Option<String>,
    pub status: Status,
    /// Index of the clip that plays now; equals the number of skips and wrong guesses.
    pub stage: usize,
    pub clips: [f64; MAX_GUESSES],
    pub full_seconds: f64,
    pub guesses: Vec<GuessView>,
    pub max_guesses: usize,
    pub answer: Option<Answer>,
    pub has_cover: bool,
    pub streak: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub title: String,
    pub artist: String,
}

/// The puzzle key for the player's settings: "mix", "random" or a genre code.
pub fn variant(settings: &SnippetSettings) -> &'static str {
    match settings.mode {
        SnippetMode::Mix => MIX,
        SnippetMode::Random => RANDOM,
        SnippetMode::Genre => settings.genre.code(),
    }
}

/// Every puzzle the generator prepares each day, so switching modes or genres works right away.
pub fn all_variants() -> Vec<&'static str> {
    [MIX, RANDOM].into_iter().chain(SnippetGenre::ALL.iter().map(|genre| genre.code())).collect()
}

struct Song {
    genre: String,
    title: String,
    artist: String,
    album: String,
    link: String,
}

fn load_song(connection: &Connection, day: &str, variant: &str) -> Result<Option<Song>, AppError> {
    Ok(connection
        .query_row("SELECT genre, title, artist, album, link FROM snippet_songs WHERE day = ?1 AND variant = ?2", params![day, variant], |row| {
            Ok(Song { genre: row.get(0)?, title: row.get(1)?, artist: row.get(2)?, album: row.get(3)?, link: row.get(4)? })
        })
        .optional()?)
}

fn load_guesses(connection: &Connection, day: &str, variant: &str) -> Result<Vec<GuessView>, AppError> {
    let mut statement = connection.prepare("SELECT kind, typed FROM snippet_guesses WHERE day = ?1 AND variant = ?2 ORDER BY attempt")?;
    let guesses = statement.query_map(params![day, variant], |row| Ok(GuessView { kind: row.get(0)?, text: row.get(1)? }))?.collect::<Result<_, _>>()?;
    Ok(guesses)
}

/// Days with a song named correctly (any mode).
pub fn streak(connection: &Connection, today: NaiveDate) -> Result<u32, AppError> {
    let mut statement = connection.prepare("SELECT DISTINCT day FROM snippet_guesses WHERE kind = ?1")?;
    let solved: Vec<String> = statement.query_map(params![RIGHT], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(day::streak(solved, today))
}

fn status_of(guesses: &[GuessView]) -> Status {
    if guesses.iter().any(|guess| guess.kind == RIGHT) {
        Status::Solved
    } else if guesses.len() >= MAX_GUESSES {
        Status::Failed
    } else if guesses.is_empty() {
        Status::New
    } else {
        Status::Progress
    }
}

fn finished(status: Status) -> bool {
    matches!(status, Status::Solved | Status::Failed)
}

pub fn state(connection: &Connection, today: NaiveDate, settings: &SnippetSettings) -> Result<State, AppError> {
    let day = format_day(today);
    let variant = variant(settings);
    let song = load_song(connection, &day, variant)?;
    let guesses = if song.is_some() { load_guesses(connection, &day, variant)? } else { Vec::new() };
    let status = if song.is_some() { status_of(&guesses) } else { Status::Waiting };
    let done = finished(status);
    let has_cover = done
        && connection
            .query_row("SELECT cover IS NOT NULL FROM snippet_audio WHERE day = ?1 AND variant = ?2", params![day, variant], |row| row.get(0))
            .optional()?
            .unwrap_or(false);
    Ok(State {
        date: day,
        variant: variant.to_owned(),
        mode: settings.mode,
        genre: song.as_ref().filter(|song| song.genre != MIX).map(|song| song.genre.clone()),
        status,
        stage: guesses.iter().filter(|guess| guess.kind != RIGHT).count().min(MAX_GUESSES - 1),
        clips: settings.clips.seconds(),
        full_seconds: FULL_SECONDS,
        guesses,
        max_guesses: MAX_GUESSES,
        answer: song.filter(|_| done).map(|song| Answer { title: song.title, artist: song.artist, album: song.album, link: song.link }),
        has_cover,
        streak: streak(connection, today)?,
    })
}

/// "Song Title (feat. X) - Remastered 2011" -> "song title", for comparing guesses with the answer.
pub fn clean_title(title: &str) -> String {
    let lower = title.to_lowercase();
    let cut = [" (", " [", " - ", " feat.", " ft.", " featuring "].iter().filter_map(|marker| lower.find(marker)).min().unwrap_or(lower.len());
    // Song titles are compared the same way in every language, so English rules are used for both sides.
    text::normalize(Language::En, &lower[..cut])
}

fn close_enough(typed: &str, expected: &str) -> bool {
    !typed.is_empty() && (typed == expected || text::edit_distance(typed, expected) <= text::typo_allowance(expected))
}

/// A guess is right when the title matches (a typo or two forgiven); an artist, if given, must match too.
pub fn is_right(typed: &str, title: &str, artist: &str) -> bool {
    let (typed_title, typed_artist) = match typed.split_once(TITLE_ARTIST_SEPARATOR.trim()) {
        Some((title_part, artist_part)) => (title_part, Some(artist_part)),
        None => (typed, None),
    };
    let title_matches = close_enough(&clean_title(typed_title), &clean_title(title));
    let artist_matches = typed_artist.is_none_or(|typed_artist| {
        let typed_artist = text::normalize(Language::En, typed_artist);
        let artist = text::normalize(Language::En, artist);
        typed_artist.is_empty() || artist.contains(&typed_artist) || close_enough(&typed_artist, &artist)
    });
    title_matches && artist_matches
}

fn record(connection: &Connection, day: &str, variant: &str, kind: &str, typed: &str) -> Result<(), AppError> {
    connection.execute(
        "INSERT INTO snippet_guesses (day, variant, attempt, kind, typed)
         SELECT ?1, ?2, COALESCE(MAX(attempt), 0) + 1, ?3, ?4 FROM snippet_guesses WHERE day = ?1 AND variant = ?2",
        params![day, variant, kind, typed],
    )?;
    Ok(())
}

/// The song for today, refusing when it is not ready or already finished.
fn playable(connection: &Connection, today: NaiveDate, settings: &SnippetSettings) -> Result<(String, &'static str, Song), AppError> {
    let day = format_day(today);
    let variant = variant(settings);
    let song = load_song(connection, &day, variant)?.ok_or_else(|| AppError::bad_request("not_ready", "Today's song is not ready yet."))?;
    if finished(status_of(&load_guesses(connection, &day, variant)?)) {
        return Err(AppError::bad_request("finished", "Today's song is already finished."));
    }
    Ok((day, variant, song))
}

pub fn guess(connection: &Connection, today: NaiveDate, typed: &str, settings: &SnippetSettings) -> Result<State, AppError> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err(AppError::bad_request("empty_guess", "Type a song first."));
    }
    if typed.chars().count() > MAX_GUESS_CHARS {
        return Err(AppError::bad_request("too_long", format!("A guess can be at most {MAX_GUESS_CHARS} characters.")));
    }
    let (day, variant, song) = playable(connection, today, settings)?;
    let kind = if is_right(typed, &song.title, &song.artist) { RIGHT } else { WRONG };
    record(connection, &day, variant, kind, typed)?;
    state(connection, today, settings)
}

pub fn skip(connection: &Connection, today: NaiveDate, settings: &SnippetSettings) -> Result<State, AppError> {
    let (day, variant, _) = playable(connection, today, settings)?;
    record(connection, &day, variant, SKIP, "")?;
    state(connection, today, settings)
}

/// The part of today's song the player may hear: the current clip, or the whole preview once finished.
pub fn clip(connection: &Connection, today: NaiveDate, settings: &SnippetSettings) -> Result<Vec<u8>, AppError> {
    let game = state(connection, today, settings)?;
    if game.status == Status::Waiting {
        return Err(AppError::bad_request("not_ready", "Today's song is not ready yet."));
    }
    let mp3: Option<Vec<u8>> = connection
        .query_row("SELECT mp3 FROM snippet_audio WHERE day = ?1 AND variant = ?2", params![game.date, game.variant], |row| row.get(0))
        .optional()?;
    let mp3 = mp3.ok_or_else(|| AppError::Corrupt(format!("the audio for {} {} is missing", game.date, game.variant)))?;
    let seconds = if finished(game.status) { FULL_SECONDS } else { game.clips[game.stage] };
    Ok(mp3::cut(&mp3, seconds).to_vec())
}

/// The album cover, only after the song is finished (it would give the answer away).
pub fn cover(connection: &Connection, today: NaiveDate, settings: &SnippetSettings) -> Result<Option<Vec<u8>>, AppError> {
    let game = state(connection, today, settings)?;
    if !finished(game.status) {
        return Err(AppError::bad_request("not_finished", "The cover is shown once the song is finished."));
    }
    Ok(connection
        .query_row("SELECT cover FROM snippet_audio WHERE day = ?1 AND variant = ?2", params![game.date, game.variant], |row| row.get(0))
        .optional()?
        .flatten())
}

/// Songs from the catalog whose title and artist contain every word typed, most popular first.
pub fn suggest(connection: &Connection, query: &str) -> Result<Vec<Suggestion>, AppError> {
    let words: Vec<String> = text::normalize(Language::En, query).split_whitespace().map(str::to_owned).collect();
    let Some(first) = words.first() else { return Ok(Vec::new()) };
    let mut statement =
        connection.prepare("SELECT title, artist, search FROM snippet_catalog WHERE instr(search, ?1) > 0 ORDER BY rank DESC LIMIT ?2")?;
    let rows = statement.query_map(params![first, SUGGESTION_SCAN], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?;
    let mut suggestions: Vec<Suggestion> = Vec::new();
    for row in rows {
        let (title, artist, search) = row?;
        let duplicate = suggestions.iter().any(|known| known.title == title && known.artist == artist);
        if !duplicate && words.iter().all(|word| search.contains(word.as_str())) {
            suggestions.push(Suggestion { title, artist });
            if suggestions.len() == SUGGESTION_LIMIT {
                break;
            }
        }
    }
    Ok(suggestions)
}

/// Home tile picture: what each guess was ("skip", "wrong", "right").
pub fn tile_marks(state: &State) -> Vec<String> {
    state.guesses.iter().map(|guess| guess.kind.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    const TODAY: &str = "2026-11-08";

    fn connection_with_song() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut connection).unwrap();
        connection
            .execute(
                "INSERT INTO snippet_songs VALUES (?1, 'mix', 'mix', 1, 'Paper Satellites', 'Mira Vale', 7, 'Small Hours', 'https://example.invalid')",
                params![TODAY],
            )
            .unwrap();
        connection.execute("INSERT INTO snippet_audio VALUES (?1, 'mix', zeroblob(30000), NULL)", params![TODAY]).unwrap();
        connection
    }

    fn today() -> NaiveDate {
        day::parse_day(TODAY).unwrap()
    }

    #[test]
    fn titles_match_without_versions_features_or_small_typos() {
        assert!(is_right("paper satelites", "Paper Satellites", "Mira Vale"));
        assert!(is_right("Paper Satellites — Mira Vale", "Paper Satellites (feat. Nobody)", "Mira Vale"));
        assert!(is_right("Bohemian Rhapsody", "Bohemian Rhapsody - Remastered 2011", "Queen"));
        assert!(!is_right("Paper Satellites — Someone Else", "Paper Satellites", "Mira Vale"));
        assert!(!is_right("Paper", "Paper Satellites", "Mira Vale"));
    }

    #[test]
    fn skips_and_wrong_guesses_unlock_longer_clips_until_the_song_is_out() {
        let connection = connection_with_song();
        let settings = SnippetSettings::default();
        let first = clip(&connection, today(), &settings).unwrap();
        skip(&connection, today(), &settings).unwrap();
        let game = guess(&connection, today(), "Neon Harbour", &settings).unwrap();
        assert_eq!(game.stage, 2);
        assert!(game.answer.is_none());
        assert!(clip(&connection, today(), &settings).unwrap().len() > first.len());
        assert!(cover(&connection, today(), &settings).is_err());
        for _ in 0..4 {
            skip(&connection, today(), &settings).unwrap();
        }
        let game = state(&connection, today(), &settings).unwrap();
        assert_eq!(game.status, Status::Failed);
        assert_eq!(game.answer.unwrap().title, "Paper Satellites");
        assert!(skip(&connection, today(), &settings).is_err());
        assert_eq!(clip(&connection, today(), &settings).unwrap().len(), 30000);
    }

    #[test]
    fn a_right_guess_solves_and_counts_for_the_streak() {
        let connection = connection_with_song();
        let settings = SnippetSettings::default();
        let game = guess(&connection, today(), "Paper Satellites", &settings).unwrap();
        assert_eq!(game.status, Status::Solved);
        assert_eq!(game.streak, 1);
        assert_eq!(tile_marks(&game), vec!["right".to_owned()]);
        let other = SnippetSettings { mode: SnippetMode::Genre, ..SnippetSettings::default() };
        assert_eq!(state(&connection, today(), &other).unwrap().status, Status::Waiting);
    }

    #[test]
    fn suggestions_need_every_word() {
        let connection = connection_with_song();
        connection.execute("INSERT INTO snippet_catalog VALUES (1, 'Paper Satellites', 'Mira Vale', 7, 'pop', 900, 'paper satellites mira vale')", []).unwrap();
        connection.execute("INSERT INTO snippet_catalog VALUES (2, 'Paperweight', 'Holloway', 8, 'rock', 950, 'paperweight holloway')", []).unwrap();
        assert_eq!(suggest(&connection, "paper").unwrap().len(), 2);
        let found = suggest(&connection, "Paper Mira").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist, "Mira Vale");
        assert!(suggest(&connection, "  ").unwrap().is_empty());
    }
}
