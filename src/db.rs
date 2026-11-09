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
    // 3: Atlas Hunt. Days, clues and answers come from the generator; plays and misses from the player.
    // Fun facts are a pool per country; shown_on marks a fact as used so it never comes back.
    r"
    CREATE TABLE atlas_days (
        day    TEXT NOT NULL,
        lang   TEXT NOT NULL,
        region TEXT NOT NULL,
        PRIMARY KEY (day, lang)
    ) STRICT;

    CREATE TABLE atlas_clues (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        kind     TEXT NOT NULL,
        clue     TEXT NOT NULL,
        PRIMARY KEY (day, lang, position),
        FOREIGN KEY (day, lang) REFERENCES atlas_days (day, lang) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE atlas_answers (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        country  TEXT NOT NULL,
        tier     INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 5),
        PRIMARY KEY (day, lang, position, country),
        FOREIGN KEY (day, lang, position) REFERENCES atlas_clues (day, lang, position) ON DELETE CASCADE
    ) STRICT;

    CREATE TABLE atlas_plays (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        outcome  TEXT NOT NULL DEFAULT 'playing' CHECK (outcome IN ('playing', 'found', 'lost')),
        country  TEXT,
        tier     INTEGER,
        points   INTEGER NOT NULL DEFAULT 0,
        fact     TEXT,
        PRIMARY KEY (day, lang, position),
        FOREIGN KEY (day, lang, position) REFERENCES atlas_clues (day, lang, position)
    ) STRICT;

    CREATE TABLE atlas_misses (
        day      TEXT NOT NULL,
        lang     TEXT NOT NULL,
        position INTEGER NOT NULL,
        attempt  INTEGER NOT NULL,
        country  TEXT NOT NULL,
        PRIMARY KEY (day, lang, position, attempt),
        FOREIGN KEY (day, lang, position) REFERENCES atlas_clues (day, lang, position)
    ) STRICT;

    CREATE TABLE atlas_facts (
        id       INTEGER PRIMARY KEY,
        lang     TEXT NOT NULL,
        country  TEXT NOT NULL,
        fact     TEXT NOT NULL,
        shown_on TEXT,
        UNIQUE (lang, country, fact)
    ) STRICT;
    CREATE INDEX atlas_facts_unseen ON atlas_facts (lang, country, shown_on);
    ",
    // 4: Snippet. The catalog feeds the guess suggestions; songs and their audio are picked by the generator.
    // A track is used at most once (unique track_id). Audio is deleted once its day is over.
    r"
    CREATE TABLE snippet_catalog (
        track_id  INTEGER PRIMARY KEY,
        title     TEXT NOT NULL,
        artist    TEXT NOT NULL,
        artist_id INTEGER NOT NULL,
        genre     TEXT NOT NULL,
        rank      INTEGER NOT NULL,
        search    TEXT NOT NULL
    ) STRICT;

    CREATE TABLE snippet_songs (
        day       TEXT NOT NULL,
        variant   TEXT NOT NULL,
        genre     TEXT NOT NULL,
        track_id  INTEGER NOT NULL UNIQUE,
        title     TEXT NOT NULL,
        artist    TEXT NOT NULL,
        artist_id INTEGER NOT NULL,
        album     TEXT NOT NULL,
        link      TEXT NOT NULL,
        PRIMARY KEY (day, variant)
    ) STRICT;

    CREATE TABLE snippet_audio (
        day     TEXT NOT NULL,
        variant TEXT NOT NULL,
        mp3     BLOB NOT NULL,
        cover   BLOB,
        PRIMARY KEY (day, variant),
        FOREIGN KEY (day, variant) REFERENCES snippet_songs (day, variant)
    ) STRICT;

    CREATE TABLE snippet_guesses (
        day     TEXT NOT NULL,
        variant TEXT NOT NULL,
        attempt INTEGER NOT NULL,
        kind    TEXT NOT NULL CHECK (kind IN ('skip', 'wrong', 'right')),
        typed   TEXT NOT NULL DEFAULT '',
        PRIMARY KEY (day, variant, attempt),
        FOREIGN KEY (day, variant) REFERENCES snippet_songs (day, variant)
    ) STRICT;
    ",
    // 5: Snippet chart sources. Variants and catalog pools get a source prefix ("world/pop", "local/mix"); everything
    // stored so far came from the local (country) charts. A track may now sit in several pools.
    r"
    PRAGMA defer_foreign_keys = ON;
    UPDATE snippet_songs SET variant = 'local/' || variant;
    UPDATE snippet_audio SET variant = 'local/' || variant;
    UPDATE snippet_guesses SET variant = 'local/' || variant;

    CREATE TABLE snippet_catalog_pools (
        track_id  INTEGER NOT NULL,
        pool      TEXT NOT NULL,
        title     TEXT NOT NULL,
        artist    TEXT NOT NULL,
        artist_id INTEGER NOT NULL,
        rank      INTEGER NOT NULL,
        search    TEXT NOT NULL,
        PRIMARY KEY (track_id, pool)
    ) STRICT;
    INSERT INTO snippet_catalog_pools (track_id, pool, title, artist, artist_id, rank, search)
        SELECT track_id, 'local/' || genre, title, artist, artist_id, rank, search FROM snippet_catalog;
    DROP TABLE snippet_catalog;
    ALTER TABLE snippet_catalog_pools RENAME TO snippet_catalog;
    ",
    // 6: when Claude's usage limit is reached, the generator waits until this time (Unix milliseconds); the web
    // app shows it. Atlas Hunt's answers get a note on why each country fits its clue, shown when it is found.
    // The clues got a new style too, so unplayed puzzles after today are made again.
    r"
    ALTER TABLE atlas_answers ADD COLUMN note TEXT;

    CREATE TABLE claude_pause (
        id       INTEGER PRIMARY KEY CHECK (id = 1),
        until_ms INTEGER NOT NULL
    ) STRICT;

    DELETE FROM atlas_days WHERE day > date('now')
        AND NOT EXISTS (SELECT 1 FROM atlas_plays p WHERE p.day = atlas_days.day AND p.lang = atlas_days.lang);
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
