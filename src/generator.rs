//! `quikflip generate`: makes sure puzzles exist for today and the next days, and judges contested answers.
//! With `--loop` it repeats every hour, so a missed night (NAS off, Claude unavailable) is caught up automatically.

use std::env;
use std::time::Duration;

use chrono::Days;
use rusqlite::Connection;

use crate::claude::Claude;
use crate::config::Config;
use crate::day::{Clock, format_day};
use crate::db;
use crate::rarefind;
use crate::settings::Language;

/// Puzzles are prepared this many days ahead, so one failed night never leaves you without a game.
const DAYS_AHEAD: u64 = 2;
const PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct PassSummary {
    generated: usize,
    failed: usize,
    contests: usize,
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

async fn pass(claude: &Claude, connection: &mut Connection, clock: &Clock, languages: &[Language]) -> PassSummary {
    let mut summary = PassSummary::default();
    let today = clock.today();
    for offset in 0..=DAYS_AHEAD {
        let day = today + Days::new(offset);
        for &language in languages {
            match rarefind::has_puzzle(connection, &format_day(day), language) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => {
                    tracing::error!("Rarefind {} {}: could not check the database: {error}", format_day(day), language.code());
                    summary.failed += 1;
                    continue;
                }
            }
            match rarefind::generate::generate(claude, connection, day, language).await {
                Ok(()) => {
                    tracing::info!("Rarefind {} {}: puzzle ready", format_day(day), language.code());
                    summary.generated += 1;
                }
                Err(error) => {
                    tracing::error!("Rarefind {} {}: {error:#}", format_day(day), language.code());
                    summary.failed += 1;
                }
            }
        }
    }
    match rarefind::generate::judge_contests(claude, connection).await {
        Ok(decided) => summary.contests = decided,
        Err(error) => {
            tracing::error!("Rarefind contests: {error:#}");
            summary.failed += 1;
        }
    }
    summary
}

pub async fn run(config: &Config, keep_running: bool) -> anyhow::Result<()> {
    let claude = Claude::from_env()?;
    let languages = languages_from_env()?;
    let clock = Clock::new(config.timezone);
    let mut connection = db::open_connection(&config.database_path())?;
    loop {
        let summary = pass(&claude, &mut connection, &clock, &languages).await;
        tracing::info!(
            "Generator pass done: {} puzzles made, {} contests judged, {} failures",
            summary.generated,
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
