mod api;
mod claude;
mod config;
mod day;
mod db;
mod error;
mod generator;
mod letterlock;
mod rarefind;
mod settings;
mod text;

use std::process::ExitCode;

use anyhow::Context;
use tokio::net::TcpListener;

use crate::api::AppState;
use crate::config::Config;
use crate::day::Clock;
use crate::db::Db;

const USAGE: &str = "\
Usage:
  quikflip serve              run the web app
  quikflip generate           make missing puzzles once, then exit (non-zero if anything failed)
  quikflip generate --loop    keep doing that every hour (the generator container)

Environment:
  QUIKFLIP_DATA_DIR                folder for the database (default: ./data next to the binary)
  QUIKFLIP_WEB_DIR                 folder with the front end (default: ./web next to the binary)
  QUIKFLIP_BIND                    address to listen on (default: 0.0.0.0:8080)
  QUIKFLIP_TIMEZONE                time zone that decides when a new day starts (default: Europe/Brussels)
  QUIKFLIP_LANGUAGES               languages to generate puzzles for (default: en,fr,nl)
  QUIKFLIP_CLAUDE_BIN              the Claude Code CLI (default: claude)
  QUIKFLIP_CLAUDE_MODEL            model for the CLI, e.g. opus or sonnet (default: the CLI's default)
  QUIKFLIP_CLAUDE_TIMEOUT_SECONDS  how long one Claude call may take (default: 900)
  CLAUDE_CODE_OAUTH_TOKEN          subscription token from `claude setup-token` (read by the CLI)";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().with_target(false).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["serve"] => serve().await,
        ["generate"] => generate(false).await,
        ["generate", "--loop"] => generate(true).await,
        ["-h" | "--help" | "help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn generate(keep_running: bool) -> anyhow::Result<()> {
    let config = Config::from_env()?;
    generator::run(&config, keep_running).await
}

async fn serve() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    config.require_web()?;
    let db = Db::open(&config.database_path())?;
    let app = api::router(AppState { db, clock: Clock::new(config.timezone) }, &config.web_dir);
    let listener = TcpListener::bind(config.bind).await.with_context(|| format!("could not listen on {}", config.bind))?;
    tracing::info!(
        "QuikFlip listening on http://{} (data: {}, web: {}, day starts at midnight {})",
        config.bind,
        config.data_dir.display(),
        config.web_dir.display(),
        config.timezone
    );
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.context("the web server stopped with an error")?;
    tracing::info!("QuikFlip stopped");
    Ok(())
}

/// Stops cleanly on Ctrl+C and on the SIGTERM that `docker stop` sends.
pub(crate) async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!("could not listen for Ctrl+C: {error}");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!("could not listen for SIGTERM: {error}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
}
