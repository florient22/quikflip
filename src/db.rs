use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use rusqlite::Connection;

use crate::error::AppError;

/// Schema changes, applied in order. Never edit a released entry; add a new one instead.
const MIGRATIONS: &[&str] = &[
    // 1: settings and Letterlock.
    r"
    CREATE TABLE settings (
        scope   TEXT PRIMARY KEY,
        version INTEGER NOT NULL,
        value   TEXT NOT NULL
    ) STRICT;

    CREATE TABLE letterlock_daily (
        day       TEXT NOT NULL,
        lang      TEXT NOT NULL,
        word      TEXT NOT NULL,
        hard_mode INTEGER,
        PRIMARY KEY (day, lang)
    ) STRICT;
    CREATE INDEX letterlock_daily_word ON letterlock_daily (lang, word);

    CREATE TABLE letterlock_guesses (
        day     TEXT NOT NULL,
        lang    TEXT NOT NULL,
        attempt INTEGER NOT NULL,
        word    TEXT NOT NULL,
        marks   TEXT NOT NULL,
        PRIMARY KEY (day, lang, attempt),
        FOREIGN KEY (day, lang) REFERENCES letterlock_daily (day, lang)
    ) STRICT;
    ",
    // 2: Rarefind. Puzzles are written by the nightly generator; plays, attempts and contests by the player.
    r"
    CREATE TABLE rarefind_prompts (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        category TEXT NOT NULL,
        prompt   TEXT NOT NULL,
        PRIMARY KEY (day, lang, position)
    ) STRICT;

    CREATE TABLE rarefind_answers (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        answer   TEXT NOT NULL,
        display  TEXT NOT NULL,
        tier     INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 5),
        PRIMARY KEY (day, lang, position, answer),
        FOREIGN KEY (day, lang, position) REFERENCES rarefind_prompts (day, lang, position)
    ) STRICT;

    CREATE TABLE rarefind_plays (
        day         TEXT NOT NULL,
        lang        TEXT NOT NULL,
        position    INTEGER NOT NULL,
        started_at  INTEGER NOT NULL,
        time_limit  INTEGER,
        penalty     INTEGER NOT NULL DEFAULT 0,
        outcome     TEXT NOT NULL DEFAULT 'playing' CHECK (outcome IN ('playing', 'answered', 'timed_out')),
        answer      TEXT,
        tier        INTEGER,
        points      INTEGER NOT NULL DEFAULT 0,
        finished_at INTEGER,
        PRIMARY KEY (day, lang, position),
        FOREIGN KEY (day, lang, position) REFERENCES rarefind_prompts (day, lang, position)
    ) STRICT;

    CREATE TABLE rarefind_attempts (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        attempt  INTEGER NOT NULL,
        typed    TEXT NOT NULL,
        PRIMARY KEY (day, lang, position, attempt),
        FOREIGN KEY (day, lang, position) REFERENCES rarefind_prompts (day, lang, position)
    ) STRICT;

    CREATE TABLE rarefind_contests (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        typed    TEXT NOT NULL,
        status   TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
        tier     INTEGER,
        reason   TEXT,
        PRIMARY KEY (day, lang, position),
        FOREIGN KEY (day, lang, position) REFERENCES rarefind_prompts (day, lang, position)
    ) STRICT;
    ",
];

/// One SQLite connection shared by all requests. QuikFlip has a single player, so one connection is plenty.
#[derive(Clone)]
pub struct Db {
    connection: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self { connection: Arc::new(Mutex::new(open_connection(path)?)) })
    }

    /// Runs blocking database work off the async runtime.
    pub async fn call<T, F>(&self, work: F) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, AppError> + Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            // A panic in another request must not lock the database forever.
            let mut guard = connection.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            work(&mut guard)
        })
        .await
        .map_err(|_| AppError::Worker)?
    }
}

/// Opens (creating if needed), configures and upgrades the database. Used by the web server and the generator,
/// which run as separate processes on the same file; WAL mode lets them share it.
pub fn open_connection(path: &Path) -> anyhow::Result<Connection> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).with_context(|| format!("could not create the data folder {}", folder.display()))?;
    }
    let mut connection = Connection::open(path).with_context(|| format!("could not open the database {}", path.display()))?;
    connection
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .context("could not configure the database")?;
    migrate(&mut connection).with_context(|| format!("could not upgrade the database {}", path.display()))?;
    Ok(connection)
}

pub fn migrate(connection: &mut Connection) -> anyhow::Result<()> {
    let stored: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(stored).with_context(|| format!("the database has an invalid schema version {stored}"))?;
    if current > MIGRATIONS.len() {
        bail!(
            "this database was created by a newer QuikFlip (schema {current}, this build knows {}); update QuikFlip instead of downgrading",
            MIGRATIONS.len()
        );
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql).with_context(|| format!("migration {} failed", index + 1))?;
        transaction.pragma_update(None, "user_version", i64::try_from(index + 1)?)?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_once_and_refuse_newer_databases() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        migrate(&mut connection).unwrap();
        connection.pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1).unwrap();
        assert!(migrate(&mut connection).is_err());
    }
}
