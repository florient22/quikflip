//! `quikflip generate`: makes sure puzzles exist for today and the next days, tops up Atlas Hunt's fun facts,
//! picks Snippet's songs and judges contested Rarefind answers. With `--loop` it repeats every hour, so a missed
//! night (NAS off, Claude or Deezer unavailable) is caught up automatically.

use std::env;
use std::time::Duration;

use chrono::{Days, NaiveDate};
use rusqlite::Connection;

use crate::claude::Claude;
use crate::config::Config;
use crate::day::{Clock, format_day};
use crate::settings::{self, Language};
use crate::snippet::deezer::Deezer;
use crate::{atlas, db, rarefind, snippet};

/// Puzzles are prepared this many days ahead, so one failed night never leaves you without a game.
const DAYS_AHEAD: u64 = 2;
const PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);

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

async fn claude_tasks(claude: &Claude, connection: &mut Connection, today: NaiveDate, languages: &[Language], summary: &mut PassSummary) {
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
            summary.record(&format!("Rarefind {label}"), made);
            let made = atlas_day(claude, connection, day, language).await;
            summary.record(&format!("Atlas Hunt {label}"), made);
        }
    }
    for &language in languages {
        match atlas::generate::top_up_facts(claude, connection, language, today).await {
            Ok(added) => summary.facts += added,
            Err(error) => summary.record(&format!("Atlas Hunt facts {}", language.code()), Err(error)),
        }
    }
    match rarefind::generate::judge_contests(claude, connection).await {
        Ok(decided) => summary.contests = decided,
        Err(error) => summary.record("Rarefind contests", Err(error)),
    }
}

/// Snippet's songs for every mode and genre (no AI). The charts are fetched only when something is missing.
async fn snippet_tasks(deezer: &Deezer, connection: &mut Connection, today: NaiveDate, summary: &mut PassSummary) {
    let mut missing = Vec::new();
    for day in days(today) {
        for variant in snippet::all_variants() {
            match snippet::generate::has_song(connection, &format_day(day), variant) {
                Ok(true) => {}
                Ok(false) => missing.push((day, variant)),
                Err(error) => summary.record(&format!("Snippet {} {variant}", format_day(day)), Err(error)),
            }
        }
    }
    if !missing.is_empty() {
        if let Err(error) = snippet::generate::refresh_catalog(deezer, connection).await {
            // Older catalog entries may still be enough, so carry on.
            tracing::warn!("Snippet: could not refresh the charts: {error:#}");
        }
        for (day, variant) in missing {
            let made = snippet::generate::generate(deezer, connection, day, variant).await.map(|()| true);
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
    claude_tasks(claude, connection, today, languages, &mut summary).await;
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
            return Ok(());
        }
        tokio::select! {
            () = tokio::time::sleep(PASS_INTERVAL) => {}
            () = crate::shutdown_signal() => {
                tracing::info!("Generator stopped");
                return Ok(());
            }
        }
    }
}
