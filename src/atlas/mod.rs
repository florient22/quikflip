//! Atlas Hunt: five geography clues a day. Pick one country per clue; rarer valid countries score more, and every
//! find comes with a fun fact you have not seen before. Puzzles come from the nightly generator.

pub mod clues;
pub mod countries;
pub mod generate;

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::day::{self, format_day};
use crate::error::AppError;
use crate::rarefind::TIER_POINTS;
use crate::settings::{AtlasRegion, AtlasSettings, Language};

pub const CLUE_COUNT: usize = 5;
pub const MAX_MISSES: usize = 3;
/// How many of the rarest valid countries are revealed once a clue is finished.
const RAREST_SHOWN: usize = 3;
const PLAYING: &str = "playing";
const FOUND: &str = "found";
const LOST: &str = "lost";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Waiting,
    New,
    Progress,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Locked,
    Ready,
    Playing,
    Found,
    Lost,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClueView {
    pub position: usize,
    pub clue: String,
    pub outcome: Outcome,
    /// How many countries fit, as a hint.
    pub valid_count: usize,
    pub misses: Vec<String>,
    pub country: Option<String>,
    pub tier: Option<u8>,
    pub points: u32,
    pub fact: Option<String>,
    /// The rarest valid countries, once the clue is finished.
    pub rarest: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub date: String,
    pub language: Language,
    /// The region today's clues cover.
    pub region: AtlasRegion,
    /// The region in the settings; when it differs from `region`, it applies from tomorrow.
    pub next_region: AtlasRegion,
    pub status: Status,
    pub current: Option<usize>,
    pub clues: Vec<ClueView>,
    pub points: u32,
    pub streak: u32,
    pub max_misses: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PickResult {
    Found { country: String, tier: u8, points: u32, fact: Option<String> },
    #[serde(rename_all = "camelCase")]
    Miss { country: String, misses_left: usize },
    Lost { country: String },
}

struct Play {
    outcome: String,
    country: Option<String>,
    tier: Option<u8>,
    points: u32,
    fact: Option<String>,
}

impl Play {
    fn finished(&self) -> bool {
        self.outcome != PLAYING
    }
}

pub fn day_region(connection: &Connection, day: &str, language: Language) -> Result<Option<AtlasRegion>, AppError> {
    let region: Option<String> = connection
        .query_row("SELECT region FROM atlas_days WHERE day = ?1 AND lang = ?2", params![day, language.code()], |row| row.get(0))
        .optional()?;
    Ok(region.map(|code| AtlasRegion::from_code(&code).unwrap_or_default()))
}

fn load_clues(connection: &Connection, day: &str, language: Language) -> Result<Vec<String>, AppError> {
    let mut statement = connection.prepare("SELECT clue FROM atlas_clues WHERE day = ?1 AND lang = ?2 ORDER BY position")?;
    let clues = statement.query_map(params![day, language.code()], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(clues)
}

fn load_plays(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, Play>, AppError> {
    let mut statement = connection.prepare("SELECT position, outcome, country, tier, points, fact FROM atlas_plays WHERE day = ?1 AND lang = ?2")?;
    let plays = statement
        .query_map(params![day, language.code()], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                Play {
                    outcome: row.get(1)?,
                    country: row.get(2)?,
                    tier: row.get::<_, Option<i64>>(3)?.map(|tier| tier as u8),
                    points: row.get::<_, i64>(4)? as u32,
                    fact: row.get(5)?,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    Ok(plays)
}

fn load_misses(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, Vec<String>>, AppError> {
    let mut statement = connection.prepare("SELECT position, country FROM atlas_misses WHERE day = ?1 AND lang = ?2 ORDER BY attempt")?;
    let mut misses: HashMap<usize, Vec<String>> = HashMap::new();
    for row in statement.query_map(params![day, language.code()], |row| Ok((row.get::<_, i64>(0)? as usize, row.get::<_, String>(1)?)))? {
        let (position, country) = row?;
        misses.entry(position).or_default().push(country);
    }
    Ok(misses)
}

/// Valid countries per clue with their tiers.
fn load_answers(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, Vec<(String, u8)>>, AppError> {
    let mut statement =
        connection.prepare("SELECT position, country, tier FROM atlas_answers WHERE day = ?1 AND lang = ?2 ORDER BY tier DESC, country")?;
    let mut answers: HashMap<usize, Vec<(String, u8)>> = HashMap::new();
    for row in statement.query_map(params![day, language.code()], |row| {
        Ok((row.get::<_, i64>(0)? as usize, row.get::<_, String>(1)?, row.get::<_, i64>(2)? as u8))
    })? {
        let (position, country, tier) = row?;
        answers.entry(position).or_default().push((country, tier));
    }
    Ok(answers)
}

fn current_position(plays: &HashMap<usize, Play>, clue_count: usize) -> Option<usize> {
    (0..clue_count).find(|position| !plays.get(position).is_some_and(Play::finished))
}

/// Days in a row where all five clues were finished (any language).
pub fn streak(connection: &Connection, today: NaiveDate) -> Result<u32, AppError> {
    let mut statement = connection.prepare("SELECT day FROM atlas_plays WHERE outcome != ?1 GROUP BY day, lang HAVING COUNT(*) >= ?2")?;
    let done: Vec<String> = statement.query_map(params![PLAYING, CLUE_COUNT as i64], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(day::streak(done, today))
}

pub fn state(connection: &Connection, today: NaiveDate, language: Language, settings: &AtlasSettings) -> Result<State, AppError> {
    let day = format_day(today);
    let streak = streak(connection, today)?;
    let Some(region) = day_region(connection, &day, language)? else {
        return Ok(State {
            date: day,
            language,
            region: settings.region,
            next_region: settings.region,
            status: Status::Waiting,
            current: None,
            clues: Vec::new(),
            points: 0,
            streak,
            max_misses: MAX_MISSES,
        });
    };
    let clues = load_clues(connection, &day, language)?;
    let plays = load_plays(connection, &day, language)?;
    let mut misses = load_misses(connection, &day, language)?;
    let answers = load_answers(connection, &day, language)?;
    let current = current_position(&plays, clues.len());
    let views: Vec<ClueView> = clues
        .into_iter()
        .enumerate()
        .map(|(position, clue)| {
            let play = plays.get(&position);
            let finished = play.is_some_and(Play::finished);
            let valid = answers.get(&position).map_or(&[][..], Vec::as_slice);
            ClueView {
                position,
                clue,
                outcome: match play {
                    Some(play) if play.outcome == FOUND => Outcome::Found,
                    Some(play) if play.outcome == LOST => Outcome::Lost,
                    Some(_) => Outcome::Playing,
                    None if Some(position) == current => Outcome::Ready,
                    None => Outcome::Locked,
                },
                valid_count: valid.len(),
                misses: misses.remove(&position).unwrap_or_default(),
                country: play.and_then(|play| play.country.clone()),
                tier: play.and_then(|play| play.tier),
                points: play.map_or(0, |play| play.points),
                fact: play.and_then(|play| play.fact.clone()),
                rarest: if finished { valid.iter().take(RAREST_SHOWN).map(|(country, _)| country.clone()).collect() } else { Vec::new() },
            }
        })
        .collect();
    let finished = plays.values().filter(|play| play.finished()).count();
    let status = match (plays.is_empty(), finished == views.len()) {
        (true, _) => Status::New,
        (false, true) => Status::Solved,
        (false, false) => Status::Progress,
    };
    let points = plays.values().map(|play| play.points).sum();
    Ok(State { date: day, language, region, next_region: settings.region, status, current, clues: views, points, streak, max_misses: MAX_MISSES })
}

/// Takes a fun fact about the country that has never been shown, and marks it as shown today.
fn take_fact(connection: &Connection, day: &str, language: Language, country: &str) -> Result<Option<String>, AppError> {
    let fact: Option<(i64, String)> = connection
        .query_row(
            "SELECT id, fact FROM atlas_facts WHERE lang = ?1 AND country = ?2 AND shown_on IS NULL ORDER BY random() LIMIT 1",
            params![language.code(), country],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((id, text)) = fact else { return Ok(None) };
    connection.execute("UPDATE atlas_facts SET shown_on = ?1 WHERE id = ?2", params![day, id])?;
    Ok(Some(text))
}

pub fn pick(
    connection: &mut Connection,
    today: NaiveDate,
    language: Language,
    position: usize,
    code: &str,
    settings: &AtlasSettings,
) -> Result<(State, PickResult), AppError> {
    let day = format_day(today);
    let Some(region) = day_region(connection, &day, language)? else {
        return Err(AppError::bad_request("not_ready", "Today's Atlas Hunt is not ready yet."));
    };
    let country = countries::by_code(code).ok_or_else(|| AppError::bad_request("unknown_country", format!("Unknown country {code:?}.")))?;
    if !country.in_region(region) {
        return Err(AppError::bad_request("outside_region", "That country is outside today's region."));
    }
    let plays = load_plays(connection, &day, language)?;
    let clue_count = load_clues(connection, &day, language)?.len();
    if current_position(&plays, clue_count) != Some(position) {
        return Err(AppError::bad_request("wrong_position", "Play the clues in order."));
    }
    let misses = load_misses(connection, &day, language)?.remove(&position).unwrap_or_default();
    let used: HashSet<&str> = plays.values().filter_map(|play| play.country.as_deref()).chain(misses.iter().map(String::as_str)).collect();
    if used.contains(code) {
        return Err(AppError::bad_request("already_tried", "You already tried that country today."));
    }
    let tier: Option<u8> = connection
        .query_row(
            "SELECT tier FROM atlas_answers WHERE day = ?1 AND lang = ?2 AND position = ?3 AND country = ?4",
            params![day, language.code(), position as i64, code],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .map(|tier| tier as u8);
    let transaction = connection.transaction()?;
    let result = match tier {
        Some(tier) => {
            let points = TIER_POINTS[usize::from(tier) - 1];
            let fact = if settings.facts { take_fact(&transaction, &day, language, code)? } else { None };
            transaction.execute(
                "INSERT INTO atlas_plays (day, lang, position, outcome, country, tier, points, fact) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (day, lang, position) DO UPDATE SET outcome = excluded.outcome, country = excluded.country,
                   tier = excluded.tier, points = excluded.points, fact = excluded.fact",
                params![day, language.code(), position as i64, FOUND, code, tier, points, fact],
            )?;
            PickResult::Found { country: code.to_owned(), tier, points, fact }
        }
        None => {
            let missed = misses.len() + 1;
            let outcome = if missed >= MAX_MISSES { LOST } else { PLAYING };
            transaction.execute(
                "INSERT INTO atlas_misses (day, lang, position, attempt, country) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![day, language.code(), position as i64, missed as i64, code],
            )?;
            transaction.execute(
                "INSERT INTO atlas_plays (day, lang, position, outcome) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (day, lang, position) DO UPDATE SET outcome = excluded.outcome",
                params![day, language.code(), position as i64, outcome],
            )?;
            if outcome == LOST {
                PickResult::Lost { country: code.to_owned() }
            } else {
                PickResult::Miss { country: code.to_owned(), misses_left: MAX_MISSES - missed }
            }
        }
    };
    transaction.commit()?;
    Ok((state(connection, today, language, settings)?, result))
}

/// Home tile picture: per clue the moon's tier, Some(0) when the clue was lost, None when not played yet.
pub fn tile_tiers(state: &State) -> Vec<Option<u8>> {
    state
        .clues
        .iter()
        .map(|clue| match clue.outcome {
            Outcome::Found => clue.tier,
            Outcome::Lost => Some(0),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    const TODAY: &str = "2026-11-08";

    fn connection_with_puzzle() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut connection).unwrap();
        connection.execute("INSERT INTO atlas_days VALUES (?1, 'en', 'europe')", params![TODAY]).unwrap();
        for position in 0..CLUE_COUNT as i64 {
            connection.execute("INSERT INTO atlas_clues VALUES (?1, 'en', ?2, 'borders:DEU', 'Borders Germany')", params![TODAY, position]).unwrap();
            for (country, tier) in [("FRA", 1), ("BEL", 2), ("LUX", 5), ("CHE", 3)] {
                connection.execute("INSERT INTO atlas_answers VALUES (?1, 'en', ?2, ?3, ?4)", params![TODAY, position, country, tier]).unwrap();
            }
        }
        connection.execute("INSERT INTO atlas_facts (lang, country, fact) VALUES ('en', 'LUX', 'A fact about Luxembourg.')", []).unwrap();
        connection
    }

    fn today() -> NaiveDate {
        day::parse_day(TODAY).unwrap()
    }

    #[test]
    fn a_find_scores_its_tier_and_uses_up_its_fact() {
        let mut connection = connection_with_puzzle();
        let settings = AtlasSettings::default();
        let (state, result) = pick(&mut connection, today(), Language::En, 0, "LUX", &settings).unwrap();
        assert_eq!(result, PickResult::Found { country: "LUX".into(), tier: 5, points: 100, fact: Some("A fact about Luxembourg.".into()) });
        assert_eq!(state.current, Some(1));
        assert_eq!(state.clues[0].rarest, vec!["LUX".to_owned(), "CHE".to_owned(), "BEL".to_owned()]);
        // The same country can't be used twice in one day, and its only fact is gone.
        let error = pick(&mut connection, today(), Language::En, 1, "LUX", &settings).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "already_tried", .. }));
        let unseen: i64 = connection.query_row("SELECT COUNT(*) FROM atlas_facts WHERE shown_on IS NULL", [], |row| row.get(0)).unwrap();
        assert_eq!(unseen, 0);
    }

    #[test]
    fn three_misses_lose_the_clue_and_other_regions_do_not_count() {
        let mut connection = connection_with_puzzle();
        let settings = AtlasSettings::default();
        let error = pick(&mut connection, today(), Language::En, 0, "BRA", &settings).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "outside_region", .. }));
        let (_, result) = pick(&mut connection, today(), Language::En, 0, "ESP", &settings).unwrap();
        assert_eq!(result, PickResult::Miss { country: "ESP".into(), misses_left: 2 });
        assert!(pick(&mut connection, today(), Language::En, 0, "ESP", &settings).is_err());
        pick(&mut connection, today(), Language::En, 0, "ITA", &settings).unwrap();
        let (state, result) = pick(&mut connection, today(), Language::En, 0, "PRT", &settings).unwrap();
        assert_eq!(result, PickResult::Lost { country: "PRT".into() });
        assert_eq!(state.clues[0].outcome, Outcome::Lost);
        assert_eq!(state.clues[0].misses.len(), 3);
        assert_eq!(state.current, Some(1));
    }

    #[test]
    fn finishing_every_clue_counts_for_the_streak() {
        let mut connection = connection_with_puzzle();
        let settings = AtlasSettings { facts: false, ..AtlasSettings::default() };
        for (position, country) in ["FRA", "BEL", "LUX", "CHE"].into_iter().enumerate() {
            pick(&mut connection, today(), Language::En, position, country, &settings).unwrap();
        }
        for country in ["ESP", "ITA", "PRT"] {
            pick(&mut connection, today(), Language::En, 4, country, &settings).unwrap();
        }
        let done = state(&connection, today(), Language::En, &settings).unwrap();
        assert_eq!(done.status, Status::Solved);
        assert_eq!(done.streak, 1);
        assert_eq!(done.clues[2].fact, None);
        assert_eq!(tile_tiers(&done), vec![Some(1), Some(2), Some(5), Some(3), Some(0)]);
    }
}
