//! `quikflip generate`: makes sure puzzles exist for today and the next days, tops up Atlas Hunt's fun facts,
//! picks Snippet's songs and judges contested Rarefind answers. With `--loop` it repeats every hour, so a missed
//! night (NAS off, Claude or Deezer unavailable) is caught up automatically. When Claude's usage limit is reached,
//! the Claude work waits until the limit resets (Snippet carries on) and starts again right after.

use std::env;
use std::time::Duration;

use chrono::{Days, NaiveDate};
use rusqlite::Connection;

use crate::claude::{Claude, ClaudeError};
use crate::config::Config;
use crate::day::{Clock, format_day};
use crate::settings::{self, Language};
use crate::snippet::deezer::Deezer;
use crate::{atlas, claude_pause, db, rarefind, snippet};

/// Puzzles are prepared this many days ahead, so one failed night never leaves you without a game.
const DAYS_AHEAD: u64 = 2;
const PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// How long to wait at a usage limit when the CLI does not say when it resets.
const LIMIT_FALLBACK_WAIT_MS: i64 = 60 * 60 * 1000;
/// Start a little after the reset, so the first call is not refused again by clock differences.
const LIMIT_RESUME_MARGIN_MS: i64 = 2 * 60 * 1000;

#[derive(Debug, Default)]
struct PassSummary {
    generated: usize,
    failed: usize,
    contests: usize,
    facts: usize,
}

impl PassSummary {
    /// Logs one task's outcome and counts it.
    fn record(&mut self, task: &str, result: anyhow::Result<bool>) {
        match result {
            Ok(true) => {
                tracing::info!("{task}: ready");
                self.generated += 1;
            }
            Ok(false) => {}
            Err(error) => {
                tracing::error!("{task}: {error:#}");
                self.failed += 1;
            }
        }
    }
}

/// When Claude may be asked again (Unix milliseconds), if this error is Claude's usage limit.
fn limit_resumes_at<T>(result: &anyhow::Result<T>, now_ms: i64) -> Option<i64> {
    let error = result.as_ref().err()?;
    match error.downcast_ref::<ClaudeError>()? {
        ClaudeError::UsageLimit { resets_at: Some(seconds) } => Some(seconds * 1000 + LIMIT_RESUME_MARGIN_MS),
        ClaudeError::UsageLimit { resets_at: None } => Some(now_ms + LIMIT_FALLBACK_WAIT_MS),
        _ => None,
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// QUIKFLIP_LANGUAGES limits which languages get puzzles (default: en,fr,nl).
fn languages_from_env() -> anyhow::Result<Vec<Language>> {
    let Ok(list) = env::var("QUIKFLIP_LANGUAGES") else { return Ok(Language::ALL.to_vec()) };
    list.split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(|code| Language::from_code(code).ok_or_else(|| anyhow::anyhow!("QUIKFLIP_LANGUAGES: unknown language {code:?} (use en, fr, nl)")))
        .collect()
}

fn days(today: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    (0..=DAYS_AHEAD).map(move |offset| today + Days::new(offset))
}

/// Rarefind for one day and language; Ok(true) when a puzzle was made.
async fn rarefind_day(claude: &Claude, connection: &mut Connection, day: NaiveDate, language: Language) -> anyhow::Result<bool> {
    if rarefind::has_puzzle(connection, &format_day(day), language)? {
        return Ok(false);
    }
    rarefind::generate::generate(claude, connection, day, language).await?;
    Ok(true)
}

/// Atlas Hunt for one day and language, in the region from the settings; Ok(true) when a puzzle was made.
async fn atlas_day(claude: &Claude, connection: &mut Connection, day: NaiveDate, language: Language) -> anyhow::Result<bool> {
    if atlas::day_region(connection, &format_day(day), language)?.is_some() {
        return Ok(false);
    }
    let region = settings::atlas(connection)?.region;
    atlas::generate::generate(claude, connection, day, language, region).await?;
    Ok(true)
}

/// Runs every Claude task. Stops at Claude's usage limit and returns when Claude may be asked again.
async fn claude_tasks(claude: &Claude, connection: &mut Connection, today: NaiveDate, languages: &[Language], summary: &mut PassSummary) -> Option<i64> {
    for &language in languages {
        // A new region setting applies from tomorrow: later puzzles for another region are made again.
        match settings::atlas(connection).map_err(anyhow::Error::from).and_then(|atlas| {
            atlas::generate::drop_other_regions(connection, today, language, atlas.region)
        }) {
            Ok(0) => {}
            Ok(removed) => tracing::info!("Atlas Hunt {}: {removed} later puzzles will be redone for the new region", language.code()),
            Err(error) => summary.record(&format!("Atlas Hunt {} region change", language.code()), Err(error)),
        }
    }
    for day in days(today) {
        for &language in languages {
            let label = format!("{} {}", format_day(day), language.code());
            let made = rarefind_day(claude, connection, day, language).await;
            if let Some(resume) = limit_resumes_at(&made, now_ms()) {
                return Some(resume);
            }
            summary.record(&format!("Rarefind {label}"), made);
            let made = atlas_day(claude, connection, day, language).await;
            if let Some(resume) = limit_resumes_at(&made, now_ms()) {
                return Some(resume);
            }
            summary.record(&format!("Atlas Hunt {label}"), made);
        }
    }
    for &language in languages {
        let added = atlas::generate::top_up_facts(claude, connection, language, today).await;
        if let Some(resume) = limit_resumes_at(&added, now_ms()) {
            return Some(resume);
        }
        match added {
            Ok(added) => summary.facts += added,
            Err(error) => summary.record(&format!("Atlas Hunt facts {}", language.code()), Err(error)),
        }
    }
    let decided = rarefind::generate::judge_contests(claude, connection).await;
    if let Some(resume) = limit_resumes_at(&decided, now_ms()) {
        return Some(resume);
    }
    match decided {
        Ok(decided) => summary.contests = decided,
        Err(error) => summary.record("Rarefind contests", Err(error)),
    }
    None
}

fn local_time(clock: &Clock, unix_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(unix_ms)
        .map_or_else(|| unix_ms.to_string(), |time| time.with_timezone(&clock.timezone()).format("%Y-%m-%d %H:%M").to_string())
}

/// The Claude part of a pass, unless an earlier usage limit has not reset yet. Records a new limit for the web app.
async fn claude_part(claude: &Claude, connection: &mut Connection, clock: &Clock, languages: &[Language], summary: &mut PassSummary) {
    match claude_pause::active_until(connection, now_ms()) {
        Ok(Some(until)) => {
            tracing::info!("Claude's usage limit: Rarefind and Atlas Hunt wait until {}", local_time(clock, until));
            return;
        }
        Ok(None) => {}
        Err(error) => summary.record("Claude pause check", Err(error.into())),
    }
    let outcome = match claude_tasks(claude, connection, clock.today(), languages, summary).await {
        Some(until) => {
            tracing::warn!("Claude's usage limit is reached; the remaining Claude work waits until {}", local_time(clock, until));
            claude_pause::set(connection, until)
        }
        None => claude_pause::clear(connection),
    };
    if let Err(error) = outcome {
        summary.record("Claude pause", Err(error.into()));
    }
}

/// How long to sleep before the next pass: the usual hour, or less when Claude's limit resets sooner.
fn next_wait(connection: &Connection) -> Duration {
    let now = now_ms();
    match claude_pause::active_until(connection, now) {
        Ok(Some(until)) => Duration::from_millis(u64::try_from(until - now).unwrap_or(0)).min(PASS_INTERVAL),
        Ok(None) => PASS_INTERVAL,
        Err(error) => {
            tracing::error!("Claude pause check: {error}");
            PASS_INTERVAL
        }
    }
}

/// Snippet's songs for every mode and genre (no AI). The charts are fetched only when something is missing.
async fn snippet_tasks(deezer: &Deezer, connection: &mut Connection, today: NaiveDate, summary: &mut PassSummary) {
    let mut missing = Vec::new();
    for day in days(today) {
        for variant in snippet::all_variants() {
            match snippet::generate::has_song(connection, &format_day(day), &variant) {
                Ok(true) => {}
                Ok(false) => missing.push((day, variant)),
                Err(error) => summary.record(&format!("Snippet {} {variant}", format_day(day)), Err(error)),
            }
        }
    }
    if !missing.is_empty() {
        // Lists that fail are logged and skipped; older catalog entries may still be enough.
        let added = snippet::generate::refresh_catalog(deezer, connection).await;
        tracing::info!("Snippet: {added} chart entries refreshed");
        for (day, variant) in missing {
            let made = snippet::generate::generate(deezer, connection, day, &variant).await.map(|()| true);
            summary.record(&format!("Snippet {} {variant}", format_day(day)), made);
        }
    }
    if let Err(error) = snippet::generate::delete_old_audio(connection, today) {
        summary.record("Snippet audio clean-up", Err(error));
    }
}

async fn pass(claude: &Claude, deezer: &Deezer, connection: &mut Connection, clock: &Clock, languages: &[Language]) -> PassSummary {
    let mut summary = PassSummary::default();
    let today = clock.today();
    snippet_tasks(deezer, connection, today, &mut summary).await;
    claude_part(claude, connection, clock, languages, &mut summary).await;
    summary
}

pub async fn run(config: &Config, keep_running: bool) -> anyhow::Result<()> {
    let claude = Claude::from_env()?;
    let deezer = Deezer::new()?;
    let languages = languages_from_env()?;
    let clock = Clock::new(config.timezone);
    let mut connection = db::open_connection(&config.database_path())?;
    loop {
        let summary = pass(&claude, &deezer, &mut connection, &clock, &languages).await;
        tracing::info!(
            "Generator pass done: {} puzzles made, {} fun facts added, {} contests judged, {} failures",
            summary.generated,
            summary.facts,
            summary.contests,
            summary.failed
        );
        if !keep_running {
            anyhow::ensure!(summary.failed == 0, "{} generator tasks failed; see the log above", summary.failed);
            if let Some(until) = claude_pause::active_until(&connection, now_ms())? {
                anyhow::bail!("Claude's usage limit is reached; run again after {}", local_time(&clock, until));
            }
            return Ok(());
        }
        tokio::select! {
            () = tokio::time::sleep(next_wait(&connection)) => {}
            () = crate::shutdown_signal() => {
                tracing::info!("Generator stopped");
                return Ok(());
            }
        }
    }
}
