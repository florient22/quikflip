//! Rarefind: seven open prompts a day; rarer valid answers score more. Puzzles come from the nightly generator.

pub mod generate;

use std::collections::{HashMap, HashSet};

use chrono::{Days, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::day::format_day;
use crate::error::AppError;
use crate::settings::{Language, RarefindSettings};
use crate::text;

pub const PROMPT_COUNT: usize = 7;
pub const TIER_POINTS: [u32; 5] = [10, 25, 50, 80, 100];
const WRONG_PENALTY_SECONDS: i64 = 3;
/// Extra time for the network round trip before an answer counts as late.
const GRACE_MS: i64 = 1500;
/// How early the browser may report "time's up" (clock drift between browser and server).
const EXPIRE_TOLERANCE_MS: i64 = 750;
const PLAYING: &str = "playing";
const ANSWERED: &str = "answered";
const TIMED_OUT: &str = "timed_out";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// No puzzle yet for this day and language: the generator has not run (or failed).
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
    Answered,
    TimedOut,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContestView {
    pub typed: String,
    pub status: String,
    pub tier: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptView {
    pub position: usize,
    pub category: String,
    /// Hidden until the player starts this prompt, so the timer is fair.
    pub prompt: Option<String>,
    pub outcome: Outcome,
    pub time_limit: Option<i64>,
    pub remaining_ms: Option<i64>,
    pub answer: Option<String>,
    pub tier: Option<u8>,
    pub points: u32,
    /// One of the rarest accepted answers, shown once the prompt is finished.
    pub rarest: Option<String>,
    pub rejected: Vec<String>,
    pub contest: Option<ContestView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub date: String,
    pub language: Language,
    pub status: Status,
    pub current: Option<usize>,
    pub prompts: Vec<PromptView>,
    pub points: u32,
    pub streak: u32,
    /// Seconds per prompt for the next prompt started (None = no timer).
    pub timer: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnswerResult {
    Accepted { answer: String, tier: u8, points: u32 },
    #[serde(rename_all = "camelCase")]
    Rejected { penalty_seconds: i64 },
    TimedOut,
}

struct Play {
    started_at: i64,
    time_limit: Option<i64>,
    penalty: i64,
    outcome: String,
    answer: Option<String>,
    tier: Option<u8>,
    points: u32,
}

impl Play {
    fn deadline(&self) -> Option<i64> {
        self.time_limit.map(|limit| self.started_at + (limit - self.penalty) * 1000)
    }

    fn finished(&self) -> bool {
        self.outcome != PLAYING
    }
}

pub fn has_puzzle(connection: &Connection, day: &str, language: Language) -> Result<bool, AppError> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM rarefind_prompts WHERE day = ?1 AND lang = ?2",
        params![day, language.code()],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn load_prompts(connection: &Connection, day: &str, language: Language) -> Result<Vec<(String, String)>, AppError> {
    let mut statement = connection.prepare("SELECT category, prompt FROM rarefind_prompts WHERE day = ?1 AND lang = ?2 ORDER BY position")?;
    let prompts = statement.query_map(params![day, language.code()], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(prompts)
}

fn load_plays(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, Play>, AppError> {
    let mut statement = connection.prepare(
        "SELECT position, started_at, time_limit, penalty, outcome, answer, tier, points FROM rarefind_plays WHERE day = ?1 AND lang = ?2",
    )?;
    let plays = statement
        .query_map(params![day, language.code()], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                Play {
                    started_at: row.get(1)?,
                    time_limit: row.get(2)?,
                    penalty: row.get(3)?,
                    outcome: row.get(4)?,
                    answer: row.get(5)?,
                    tier: row.get::<_, Option<i64>>(6)?.map(|tier| tier as u8),
                    points: row.get::<_, i64>(7)? as u32,
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    Ok(plays)
}

/// Marks prompts whose time ran out (plus grace) as timed out; the browser may have closed before telling us.
fn expire_overdue(connection: &Connection, day: &str, language: Language, now_ms: i64) -> Result<(), AppError> {
    connection.execute(
        "UPDATE rarefind_plays SET outcome = ?1, finished_at = started_at + (time_limit - penalty) * 1000
         WHERE day = ?2 AND lang = ?3 AND outcome = ?4 AND time_limit IS NOT NULL
           AND ?5 > started_at + (time_limit - penalty) * 1000 + ?6",
        params![TIMED_OUT, day, language.code(), PLAYING, now_ms, GRACE_MS],
    )?;
    Ok(())
}

fn current_position(plays: &HashMap<usize, Play>, prompt_count: usize) -> Option<usize> {
    (0..prompt_count).find(|position| !plays.get(position).is_some_and(Play::finished))
}

fn rejected_attempts(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, Vec<String>>, AppError> {
    let mut statement = connection.prepare("SELECT position, typed FROM rarefind_attempts WHERE day = ?1 AND lang = ?2 ORDER BY attempt")?;
    let mut attempts: HashMap<usize, Vec<String>> = HashMap::new();
    for row in statement.query_map(params![day, language.code()], |row| Ok((row.get::<_, i64>(0)? as usize, row.get::<_, String>(1)?)))? {
        let (position, typed) = row?;
        attempts.entry(position).or_default().push(typed);
    }
    Ok(attempts)
}

fn contests(connection: &Connection, day: &str, language: Language) -> Result<HashMap<usize, ContestView>, AppError> {
    let mut statement = connection.prepare("SELECT position, typed, status, tier FROM rarefind_contests WHERE day = ?1 AND lang = ?2")?;
    let contests = statement
        .query_map(params![day, language.code()], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                ContestView { typed: row.get(1)?, status: row.get(2)?, tier: row.get::<_, Option<i64>>(3)?.map(|tier| tier as u8) },
            ))
        })?
        .collect::<Result<_, _>>()?;
    Ok(contests)
}

fn rarest_answer(connection: &Connection, day: &str, language: Language, position: usize) -> Result<Option<String>, AppError> {
    Ok(connection
        .query_row(
            "SELECT display FROM rarefind_answers WHERE day = ?1 AND lang = ?2 AND position = ?3 ORDER BY tier DESC, display LIMIT 1",
            params![day, language.code(), position as i64],
            |row| row.get(0),
        )
        .optional()?)
}

/// Days in a row where all seven prompts were played (any language), counting today only once it is done.
pub fn streak(connection: &Connection, today: NaiveDate) -> Result<u32, AppError> {
    let mut statement = connection.prepare(
        "SELECT day FROM rarefind_plays WHERE outcome != ?1 GROUP BY day, lang HAVING COUNT(*) >= ?2",
    )?;
    let done: HashSet<NaiveDate> = statement
        .query_map(params![PLAYING, PROMPT_COUNT as i64], |row| row.get::<_, String>(0))?
        .filter_map(|day| day.ok().and_then(|text| NaiveDate::parse_from_str(&text, "%Y-%m-%d").ok()))
        .collect();
    let mut day = if done.contains(&today) { today } else { today - Days::new(1) };
    let mut count = 0;
    while done.contains(&day) {
        count += 1;
        day = day - Days::new(1);
    }
    Ok(count)
}

pub fn state(connection: &Connection, today: NaiveDate, language: Language, settings: &RarefindSettings, now_ms: i64) -> Result<State, AppError> {
    let day = format_day(today);
    let prompts = load_prompts(connection, &day, language)?;
    let streak = streak(connection, today)?;
    let timer = settings.timer.seconds();
    if prompts.is_empty() {
        return Ok(State { date: day, language, status: Status::Waiting, current: None, prompts: Vec::new(), points: 0, streak, timer });
    }
    expire_overdue(connection, &day, language, now_ms)?;
    let plays = load_plays(connection, &day, language)?;
    let mut attempts = rejected_attempts(connection, &day, language)?;
    let mut contests = contests(connection, &day, language)?;
    let current = current_position(&plays, prompts.len());
    let mut views = Vec::with_capacity(prompts.len());
    for (position, (category, prompt)) in prompts.into_iter().enumerate() {
        let play = plays.get(&position);
        let finished = play.is_some_and(Play::finished);
        let outcome = match play {
            Some(play) if play.outcome == ANSWERED => Outcome::Answered,
            Some(play) if play.outcome == TIMED_OUT => Outcome::TimedOut,
            Some(_) => Outcome::Playing,
            None if Some(position) == current => Outcome::Ready,
            None => Outcome::Locked,
        };
        views.push(PromptView {
            position,
            category,
            prompt: play.map(|_| prompt),
            outcome,
            time_limit: play.and_then(|play| play.time_limit),
            remaining_ms: play.filter(|play| !play.finished()).and_then(Play::deadline).map(|deadline| (deadline - now_ms).max(0)),
            answer: play.and_then(|play| play.answer.clone()),
            tier: play.and_then(|play| play.tier),
            points: play.map_or(0, |play| play.points),
            rarest: if finished { rarest_answer(connection, &day, language, position)? } else { None },
            rejected: if finished { attempts.remove(&position).unwrap_or_default() } else { Vec::new() },
            contest: contests.remove(&position),
        });
    }
    let finished_count = plays.values().filter(|play| play.finished()).count();
    let status = match (plays.is_empty(), finished_count == views.len()) {
        (true, _) => Status::New,
        (false, true) => Status::Solved,
        (false, false) => Status::Progress,
    };
    let points = plays.values().map(|play| play.points).sum();
    Ok(State { date: day, language, status, current, prompts: views, points, streak, timer })
}

pub fn start(
    connection: &Connection,
    today: NaiveDate,
    language: Language,
    position: usize,
    settings: &RarefindSettings,
    now_ms: i64,
) -> Result<State, AppError> {
    let day = format_day(today);
    if !has_puzzle(connection, &day, language)? {
        return Err(AppError::bad_request("not_ready", "Today's Rarefind is not ready yet."));
    }
    expire_overdue(connection, &day, language, now_ms)?;
    let plays = load_plays(connection, &day, language)?;
    let prompt_count = load_prompts(connection, &day, language)?.len();
    match plays.get(&position) {
        // Starting twice (a double click or a reload) just returns the running prompt.
        Some(play) if !play.finished() => {}
        Some(_) => return Err(AppError::bad_request("already_played", "That prompt is already finished.")),
        None if Some(position) != current_position(&plays, prompt_count) => {
            return Err(AppError::bad_request("wrong_position", "Play the prompts in order."));
        }
        None => {
            connection.execute(
                "INSERT INTO rarefind_plays (day, lang, position, started_at, time_limit) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![day, language.code(), position as i64, now_ms, settings.timer.seconds()],
            )?;
        }
    }
    state(connection, today, language, settings, now_ms)
}

/// The best accepted answer for what was typed: exact (including plurals and aliases), else within a typo or two.
pub fn find_match(language: Language, typed: &str, answers: &[(String, String, u8)]) -> Option<(String, u8)> {
    let input = text::normalize(language, typed);
    let input_forms = text::variants(language, &input);
    let exact = answers
        .iter()
        .filter(|(answer, _, _)| text::variants(language, answer).iter().any(|form| input_forms.contains(form)))
        .max_by_key(|(_, _, tier)| *tier);
    if let Some((_, display, tier)) = exact {
        return Some((display.clone(), *tier));
    }
    answers
        .iter()
        .filter_map(|(answer, display, tier)| {
            let distance = text::edit_distance(&input, answer);
            (distance <= text::typo_allowance(answer)).then_some((distance, *tier, display))
        })
        .min_by_key(|(distance, tier, _)| (*distance, std::cmp::Reverse(*tier)))
        .map(|(_, tier, display)| (display.clone(), tier))
}

fn answers_for(connection: &Connection, day: &str, language: Language, position: usize) -> Result<Vec<(String, String, u8)>, AppError> {
    let mut statement = connection.prepare("SELECT answer, display, tier FROM rarefind_answers WHERE day = ?1 AND lang = ?2 AND position = ?3")?;
    let answers = statement
        .query_map(params![day, language.code(), position as i64], |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u8)))?
        .collect::<Result<_, _>>()?;
    Ok(answers)
}

pub fn answer(
    connection: &mut Connection,
    today: NaiveDate,
    language: Language,
    position: usize,
    typed: &str,
    settings: &RarefindSettings,
    now_ms: i64,
) -> Result<(State, AnswerResult), AppError> {
    let day = format_day(today);
    expire_overdue(connection, &day, language, now_ms)?;
    let plays = load_plays(connection, &day, language)?;
    let play = plays.get(&position).ok_or_else(|| AppError::bad_request("not_started", "Start the prompt first."))?;
    if play.outcome == TIMED_OUT {
        return Ok((state(connection, today, language, settings, now_ms)?, AnswerResult::TimedOut));
    }
    if play.outcome == ANSWERED {
        return Err(AppError::bad_request("already_played", "That prompt is already finished."));
    }
    if text::normalize(language, typed).is_empty() {
        return Err(AppError::bad_request("empty_answer", "Type an answer first."));
    }
    let result = match find_match(language, typed, &answers_for(connection, &day, language, position)?) {
        Some((display, tier)) => {
            let points = TIER_POINTS[usize::from(tier) - 1];
            connection.execute(
                "UPDATE rarefind_plays SET outcome = ?1, answer = ?2, tier = ?3, points = ?4, finished_at = ?5
                 WHERE day = ?6 AND lang = ?7 AND position = ?8",
                params![ANSWERED, display, tier, points, now_ms, day, language.code(), position as i64],
            )?;
            AnswerResult::Accepted { answer: display, tier, points }
        }
        None => {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT INTO rarefind_attempts (day, lang, position, attempt, typed)
                 SELECT ?1, ?2, ?3, COALESCE(MAX(attempt), 0) + 1, ?4 FROM rarefind_attempts WHERE day = ?1 AND lang = ?2 AND position = ?3",
                params![day, language.code(), position as i64, typed.trim()],
            )?;
            transaction.execute(
                "UPDATE rarefind_plays SET penalty = penalty + ?1 WHERE day = ?2 AND lang = ?3 AND position = ?4",
                params![WRONG_PENALTY_SECONDS, day, language.code(), position as i64],
            )?;
            transaction.commit()?;
            AnswerResult::Rejected { penalty_seconds: WRONG_PENALTY_SECONDS }
        }
    };
    Ok((state(connection, today, language, settings, now_ms)?, result))
}

/// The browser says time is up. Accepted only if the deadline really has passed (with a little clock tolerance).
pub fn expire(
    connection: &Connection,
    today: NaiveDate,
    language: Language,
    position: usize,
    settings: &RarefindSettings,
    now_ms: i64,
) -> Result<State, AppError> {
    let day = format_day(today);
    let plays = load_plays(connection, &day, language)?;
    if let Some(deadline) = plays.get(&position).filter(|play| !play.finished()).and_then(Play::deadline)
        && now_ms + EXPIRE_TOLERANCE_MS >= deadline
    {
        connection.execute(
            "UPDATE rarefind_plays SET outcome = ?1, finished_at = ?2 WHERE day = ?3 AND lang = ?4 AND position = ?5",
            params![TIMED_OUT, now_ms, day, language.code(), position as i64],
        )?;
    }
    state(connection, today, language, settings, now_ms)
}

/// Asks tonight's generator to take a second look at a rejected answer.
pub fn contest(
    connection: &Connection,
    today: NaiveDate,
    language: Language,
    position: usize,
    typed: &str,
    settings: &RarefindSettings,
    now_ms: i64,
) -> Result<State, AppError> {
    let day = format_day(today);
    let rejected = rejected_attempts(connection, &day, language)?.remove(&position).unwrap_or_default();
    let typed = typed.trim();
    if !rejected.iter().any(|attempt| attempt == typed) {
        return Err(AppError::bad_request("not_rejected", "Only rejected answers can be contested."));
    }
    let inserted = connection.execute(
        "INSERT INTO rarefind_contests (day, lang, position, typed) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
        params![day, language.code(), position as i64, typed],
    )?;
    if inserted == 0 {
        return Err(AppError::bad_request("already_contested", "This prompt already has a contested answer."));
    }
    state(connection, today, language, settings, now_ms)
}

/// Home tile picture: per prompt the floor tier, Some(0) when time ran out, None when not played yet.
pub fn tile_tiers(state: &State) -> Vec<Option<u8>> {
    state
        .prompts
        .iter()
        .map(|prompt| match prompt.outcome {
            Outcome::Answered => prompt.tier,
            Outcome::TimedOut => Some(0),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::migrate;

    pub(crate) fn connection_with_puzzle(day: &str) -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut connection).unwrap();
        for position in 0..PROMPT_COUNT as i64 {
            connection
                .execute("INSERT INTO rarefind_prompts VALUES (?1, 'en', ?2, 'tools and DIY', 'Something in a toolbox')", params![day, position])
                .unwrap();
            for (answer, tier) in [("hammer", 1), ("screwdriver", 1), ("plumb bob", 5), ("spirit level", 3)] {
                connection.execute("INSERT INTO rarefind_answers VALUES (?1, 'en', ?2, ?3, ?3, ?4)", params![day, position, answer, tier]).unwrap();
            }
        }
        connection
    }

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    const NOW: i64 = 1_000_000;

    #[test]
    fn matching_forgives_plurals_articles_and_typos() {
        let answers = vec![("hammer".to_owned(), "hammer".to_owned(), 1), ("plumb bob".to_owned(), "plumb bob".to_owned(), 5)];
        assert_eq!(find_match(Language::En, "Hammers", &answers), Some(("hammer".into(), 1)));
        assert_eq!(find_match(Language::En, "a plumb-bob", &answers), Some(("plumb bob".into(), 5)));
        assert_eq!(find_match(Language::En, "hamer", &answers), Some(("hammer".into(), 1)));
        assert_eq!(find_match(Language::En, "wrench", &answers), None);
    }

    #[test]
    fn waiting_without_a_puzzle() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        let state = state(&connection, day("2026-11-08"), Language::En, &RarefindSettings::default(), NOW).unwrap();
        assert_eq!(state.status, Status::Waiting);
    }

    #[test]
    fn prompts_are_played_in_order_with_a_server_side_timer() {
        let mut connection = connection_with_puzzle("2026-11-08");
        let today = day("2026-11-08");
        let settings = RarefindSettings::default();
        let error = start(&connection, today, Language::En, 1, &settings, NOW).unwrap_err();
        assert!(matches!(error, AppError::BadRequest { code: "wrong_position", .. }));
        let state = start(&connection, today, Language::En, 0, &settings, NOW).unwrap();
        assert_eq!(state.prompts[0].outcome, Outcome::Playing);
        assert_eq!(state.prompts[0].remaining_ms, Some(25_000));
        assert_eq!(state.prompts[1].prompt, None);

        let (state, result) = answer(&mut connection, today, Language::En, 0, "wrench", &settings, NOW + 1000).unwrap();
        assert_eq!(result, AnswerResult::Rejected { penalty_seconds: 3 });
        assert_eq!(state.prompts[0].remaining_ms, Some(21_000));

        let (state, result) = answer(&mut connection, today, Language::En, 0, "plumb bob", &settings, NOW + 2000).unwrap();
        assert_eq!(result, AnswerResult::Accepted { answer: "plumb bob".into(), tier: 5, points: 100 });
        assert_eq!(state.points, 100);
        assert_eq!(state.current, Some(1));
        assert_eq!(state.prompts[0].rejected, vec!["wrench".to_owned()]);
    }

    #[test]
    fn late_answers_time_out() {
        let mut connection = connection_with_puzzle("2026-11-08");
        let today = day("2026-11-08");
        let settings = RarefindSettings::default();
        start(&connection, today, Language::En, 0, &settings, NOW).unwrap();
        let (state, result) = answer(&mut connection, today, Language::En, 0, "hammer", &settings, NOW + 27_000).unwrap();
        assert_eq!(result, AnswerResult::TimedOut);
        assert_eq!(state.prompts[0].outcome, Outcome::TimedOut);
        assert_eq!(state.prompts[0].rarest.as_deref(), Some("plumb bob"));
    }

    #[test]
    fn a_full_day_counts_for_the_streak_and_contests_need_a_rejection() {
        let mut connection = connection_with_puzzle("2026-11-08");
        let today = day("2026-11-08");
        let settings = RarefindSettings::default();
        for position in 0..PROMPT_COUNT {
            start(&connection, today, Language::En, position, &settings, NOW).unwrap();
            if position == 0 {
                answer(&mut connection, today, Language::En, position, "monkey wrench", &settings, NOW).unwrap();
            }
            answer(&mut connection, today, Language::En, position, "hammer", &settings, NOW + 500).unwrap();
        }
        let done = state(&connection, today, Language::En, &settings, NOW).unwrap();
        assert_eq!(done.status, Status::Solved);
        assert_eq!(done.streak, 1);
        assert!(contest(&connection, today, Language::En, 1, "anything", &settings, NOW).is_err());
        let contested = contest(&connection, today, Language::En, 0, "monkey wrench", &settings, NOW).unwrap();
        assert_eq!(contested.prompts[0].contest.as_ref().unwrap().status, "pending");
        assert!(contest(&connection, today, Language::En, 0, "monkey wrench", &settings, NOW).is_err());
    }
}
