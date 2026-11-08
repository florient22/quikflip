use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use chrono::NaiveDate;
use rand::seq::IndexedRandom;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::day::{self, format_day};
use crate::error::AppError;
use crate::settings::{Language, LetterlockSettings};

pub const WORD_LENGTH: usize = 5;
pub const MAX_GUESSES: usize = 6;
const RIGHT_SPOT: char = 'g';
const WRONG_SPOT: char = 'y';
const NOT_IN_WORD: char = 'x';

/// Word lists are built by tools/build_wordlists.py and compiled into the binary.
struct WordList {
    answers: Vec<&'static str>,
    allowed: HashSet<&'static str>,
}

fn word_list(language: Language) -> &'static WordList {
    static LISTS: OnceLock<HashMap<Language, WordList>> = OnceLock::new();
    let lists = LISTS.get_or_init(|| {
        let build = |answers: &'static str, allowed: &'static str| {
            let answers: Vec<&'static str> = answers.lines().map(str::trim).filter(|word| !word.is_empty()).collect();
            let mut allowed: HashSet<&'static str> = allowed.lines().map(str::trim).filter(|word| !word.is_empty()).collect();
            allowed.extend(answers.iter().copied());
            WordList { answers, allowed }
        };
        HashMap::from([
            (Language::En, build(include_str!("../words/en-answers.txt"), include_str!("../words/en-allowed.txt"))),
            (Language::Fr, build(include_str!("../words/fr-answers.txt"), include_str!("../words/fr-allowed.txt"))),
            (Language::Nl, build(include_str!("../words/nl-answers.txt"), include_str!("../words/nl-allowed.txt"))),
        ])
    });
    &lists[&language]
}

/// g = right letter, right spot; y = in the word, elsewhere; x = not in the word. Repeated letters are only
/// marked as often as they occur in the answer.
pub fn score(guess: &str, answer: &str) -> String {
    let guess: Vec<char> = guess.chars().collect();
    let answer: Vec<char> = answer.chars().collect();
    let mut marks = vec![NOT_IN_WORD; guess.len()];
    let mut unmatched: HashMap<char, usize> = HashMap::new();
    for (index, (&letter, &target)) in guess.iter().zip(&answer).enumerate() {
        if letter == target {
            marks[index] = RIGHT_SPOT;
        } else {
            *unmatched.entry(target).or_default() += 1;
        }
    }
    for (index, letter) in guess.iter().enumerate() {
        if marks[index] == RIGHT_SPOT {
            continue;
        }
        if let Some(count) = unmatched.get_mut(letter).filter(|count| **count > 0) {
            marks[index] = WRONG_SPOT;
            *count -= 1;
        }
    }
    marks.into_iter().collect()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Guess {
    pub word: String,
    pub marks: String,
}

impl Guess {
    fn is_win(&self) -> bool {
        self.marks.chars().all(|mark| mark == RIGHT_SPOT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Progress,
    Solved,
    Failed,
}

pub fn status_of(guesses: &[Guess]) -> Status {
    match guesses.last() {
        None => Status::New,
        Some(last) if last.is_win() => Status::Solved,
        Some(_) if guesses.len() >= MAX_GUESSES => Status::Failed,
        Some(_) => Status::Progress,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Violation {
    /// A letter already found in the right spot must stay there.
    Green { position: usize, letter: char },
    /// A letter known to be in the word must be used.
    Present { letter: char },
}

pub fn hard_mode_violation(previous: &[Guess], word: &str) -> Option<Violation> {
    let letters: Vec<char> = word.chars().collect();
    for guess in previous {
        for (position, (letter, mark)) in guess.word.chars().zip(guess.marks.chars()).enumerate() {
            if mark == RIGHT_SPOT && letters.get(position) != Some(&letter) {
                return Some(Violation::Green { position, letter });
            }
        }
    }
    let mut required: HashMap<char, usize> = HashMap::new();
    for guess in previous {
        let mut found: HashMap<char, usize> = HashMap::new();
        for (letter, mark) in guess.word.chars().zip(guess.marks.chars()) {
            if mark != NOT_IN_WORD {
                *found.entry(letter).or_default() += 1;
            }
        }
        for (letter, count) in found {
            let entry = required.entry(letter).or_default();
            *entry = (*entry).max(count);
        }
    }
    let mut missing: Vec<char> = required
        .into_iter()
        .filter(|(letter, count)| letters.iter().filter(|candidate| *candidate == letter).count() < *count)
        .map(|(letter, _)| letter)
        .collect();
    missing.sort_unstable();
    missing.first().map(|&letter| Violation::Present { letter })
}

fn daily_word(connection: &Connection, day: &str, language: Language) -> Result<String, AppError> {
    let lang = language.code();
    if let Some(word) = connection
        .query_row("SELECT word FROM letterlock_daily WHERE day = ?1 AND lang = ?2", params![day, lang], |row| row.get(0))
        .optional()?
    {
        return Ok(word);
    }
    let answers = &word_list(language).answers;
    let mut statement = connection.prepare("SELECT word FROM letterlock_daily WHERE lang = ?1")?;
    let used: HashSet<String> = statement.query_map(params![lang], |row| row.get(0))?.collect::<Result<_, _>>()?;
    let fresh: Vec<&str> = answers.iter().copied().filter(|word| !used.contains(*word)).collect();
    let word = match fresh.choose(&mut rand::rng()) {
        Some(word) => (*word).to_owned(),
        // Every answer has been used: bring back the one that has rested longest.
        None => connection
            .query_row(
                "SELECT word FROM letterlock_daily WHERE lang = ?1 GROUP BY word ORDER BY MAX(day) ASC LIMIT 1",
                params![lang],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::Corrupt(format!("the {lang} answer list is empty")))?,
    };
    connection.execute("INSERT INTO letterlock_daily (day, lang, word) VALUES (?1, ?2, ?3)", params![day, lang, word])?;
    Ok(word)
}

fn load_guesses(connection: &Connection, day: &str, language: Language) -> Result<Vec<Guess>, AppError> {
    let mut statement = connection.prepare("SELECT word, marks FROM letterlock_guesses WHERE day = ?1 AND lang = ?2 ORDER BY attempt")?;
    let guesses = statement
        .query_map(params![day, language.code()], |row| Ok(Guess { word: row.get(0)?, marks: row.get(1)? }))?
        .collect::<Result<_, _>>()?;
    Ok(guesses)
}

fn stored_hard_mode(connection: &Connection, day: &str, language: Language) -> Result<Option<bool>, AppError> {
    let value: Option<Option<i64>> = connection
        .query_row("SELECT hard_mode FROM letterlock_daily WHERE day = ?1 AND lang = ?2", params![day, language.code()], |row| row.get(0))
        .optional()?;
    Ok(value.flatten().map(|flag| flag != 0))
}

/// Days in a row with a solved word (any language), counting today only once it is solved.
pub fn streak(connection: &Connection, today: NaiveDate) -> Result<u32, AppError> {
    let mut statement = connection.prepare("SELECT DISTINCT day FROM letterlock_guesses WHERE marks = ?1")?;
    let win = RIGHT_SPOT.to_string().repeat(WORD_LENGTH);
    let won: Vec<String> = statement.query_map(params![win], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(day::streak(won, today))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub date: String,
    pub language: Language,
    pub word_length: usize,
    pub max_guesses: usize,
    pub guesses: Vec<Guess>,
    pub status: Status,
    /// Only sent once the game is over, so the answer never reaches the browser early.
    pub answer: Option<String>,
    pub hard_mode: bool,
    pub keyboard: &'static str,
    pub streak: u32,
}

pub fn state(connection: &Connection, today: NaiveDate, language: Language, settings: &LetterlockSettings) -> Result<State, AppError> {
    let day = format_day(today);
    let answer = daily_word(connection, &day, language)?;
    let guesses = load_guesses(connection, &day, language)?;
    let status = status_of(&guesses);
    let finished = matches!(status, Status::Solved | Status::Failed);
    Ok(State {
        date: day.clone(),
        language,
        word_length: WORD_LENGTH,
        max_guesses: MAX_GUESSES,
        hard_mode: stored_hard_mode(connection, &day, language)?.unwrap_or(settings.hard_mode),
        guesses,
        status,
        answer: finished.then_some(answer),
        keyboard: language.keyboard(),
        streak: streak(connection, today)?,
    })
}

pub fn submit(
    connection: &mut Connection,
    today: NaiveDate,
    language: Language,
    typed: &str,
    settings: &LetterlockSettings,
) -> Result<State, AppError> {
    let word = typed.trim().to_lowercase();
    if word.chars().count() != WORD_LENGTH {
        return Err(AppError::bad_request("wrong_length", format!("A guess must have {WORD_LENGTH} letters.")));
    }
    if !word.chars().all(|letter| letter.is_ascii_lowercase()) {
        return Err(AppError::bad_request("invalid_letters", "A guess may only use the letters A to Z."));
    }
    let day = format_day(today);
    let answer = daily_word(connection, &day, language)?;
    let previous = load_guesses(connection, &day, language)?;
    if matches!(status_of(&previous), Status::Solved | Status::Failed) {
        return Err(AppError::bad_request("game_over", "Today's word is already finished."));
    }
    if !word_list(language).allowed.contains(word.as_str()) {
        return Err(AppError::bad_request("not_a_word", "That word is not in the word list."));
    }
    let stored = stored_hard_mode(connection, &day, language)?;
    if stored.unwrap_or(settings.hard_mode) {
        match hard_mode_violation(&previous, &word) {
            Some(Violation::Green { position, letter }) => {
                return Err(AppError::bad_request_with(
                    "hard_mode_green",
                    format!("Letter {} must be {}.", position + 1, letter.to_ascii_uppercase()),
                    json!({ "position": position + 1, "letter": letter.to_ascii_uppercase().to_string() }),
                ));
            }
            Some(Violation::Present { letter }) => {
                return Err(AppError::bad_request_with(
                    "hard_mode_present",
                    format!("The guess must contain {}.", letter.to_ascii_uppercase()),
                    json!({ "letter": letter.to_ascii_uppercase().to_string() }),
                ));
            }
            None => {}
        }
    }
    let transaction = connection.transaction()?;
    if stored.is_none() {
        // Hard mode is locked in by the first guess; changing the setting mid-game applies tomorrow.
        transaction.execute(
            "UPDATE letterlock_daily SET hard_mode = ?1 WHERE day = ?2 AND lang = ?3",
            params![settings.hard_mode, day, language.code()],
        )?;
    }
    transaction.execute(
        "INSERT INTO letterlock_guesses (day, lang, attempt, word, marks) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![day, language.code(), previous.len() as i64 + 1, word, score(&word, &answer)],
    )?;
    transaction.commit()?;
    state(connection, today, language, settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut connection).unwrap();
        connection
    }

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn force_word(connection: &Connection, date: &str, word: &str) {
        connection.execute("INSERT INTO letterlock_daily (day, lang, word) VALUES (?1, 'en', ?2)", params![date, word]).unwrap();
    }

    #[test]
    fn scoring_handles_repeated_letters() {
        assert_eq!(score("globe", "globe"), "ggggg");
        assert_eq!(score("crane", "globe"), "xxxxg");
        assert_eq!(score("eerie", "crane"), "xxyxg");
        assert_eq!(score("llama", "hello"), "yyxxx");
    }

    #[test]
    fn hard_mode_requires_known_letters() {
        let previous = vec![Guess { word: "stole".into(), marks: "xxgyg".into() }];
        assert_eq!(hard_mode_violation(&previous, "globe"), None);
        assert_eq!(hard_mode_violation(&previous, "glade"), Some(Violation::Green { position: 2, letter: 'o' }));
        assert_eq!(hard_mode_violation(&previous, "proxe"), Some(Violation::Present { letter: 'l' }));
    }

    #[test]
    fn word_lists_are_valid() {
        for language in Language::ALL {
            let list = word_list(language);
            assert!(list.answers.len() > 1000, "{language:?} has too few answers");
            for word in list.answers.iter().chain(list.allowed.iter()) {
                assert!(word.len() == WORD_LENGTH && word.chars().all(|letter| letter.is_ascii_lowercase()), "bad word {word:?}");
            }
        }
    }

    #[test]
    fn daily_word_is_stable_and_never_repeats() {
        let connection = connection();
        let first = daily_word(&connection, "2026-11-08", Language::En).unwrap();
        assert_eq!(daily_word(&connection, "2026-11-08", Language::En).unwrap(), first);
        let second = daily_word(&connection, "2026-11-09", Language::En).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn a_full_game_can_be_won_once() {
        let mut connection = connection();
        force_word(&connection, "2026-11-08", "globe");
        let settings = LetterlockSettings::default();
        let today = day("2026-11-08");
        let error = submit(&mut connection, today, Language::En, "zzzzz", &settings).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "not_a_word", .. }));
        let state = submit(&mut connection, today, Language::En, "crane", &settings).unwrap();
        assert_eq!(state.status, Status::Progress);
        assert_eq!(state.answer, None);
        let state = submit(&mut connection, today, Language::En, "GLOBE", &settings).unwrap();
        assert_eq!(state.status, Status::Solved);
        assert_eq!(state.answer.as_deref(), Some("globe"));
        assert_eq!(state.streak, 1);
        let error = submit(&mut connection, today, Language::En, "crane", &settings).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "game_over", .. }));
    }

    #[test]
    fn hard_mode_is_locked_in_by_the_first_guess() {
        let mut connection = connection();
        force_word(&connection, "2026-11-08", "globe");
        let today = day("2026-11-08");
        let hard = LetterlockSettings { hard_mode: true, ..Default::default() };
        submit(&mut connection, today, Language::En, "stole", &hard).unwrap();
        // Turning hard mode off mid-game does not apply until tomorrow.
        let error = submit(&mut connection, today, Language::En, "crane", &LetterlockSettings::default()).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "hard_mode_green", .. }));
    }

    #[test]
    fn streak_counts_consecutive_solved_days() {
        let mut connection = connection();
        let settings = LetterlockSettings::default();
        for date in ["2026-11-05", "2026-11-06", "2026-11-07"] {
            force_word(&connection, date, "globe");
            submit(&mut connection, day(date), Language::En, "globe", &settings).unwrap();
        }
        // Today (the 8th) is not played yet, so the streak still counts up to yesterday.
        assert_eq!(streak(&connection, day("2026-11-08")).unwrap(), 3);
        assert_eq!(streak(&connection, day("2026-11-10")).unwrap(), 0);
    }
}
