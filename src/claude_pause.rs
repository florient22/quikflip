//! When Claude's usage limit is reached, the generator stops asking until the limit resets. The time is kept in the
//! database, so the web app can tell the player when new puzzles will come.

use rusqlite::{Connection, OptionalExtension, params};

/// Claude may be asked again from `until_ms` (Unix milliseconds) on.
pub fn set(connection: &Connection, until_ms: i64) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO claude_pause (id, until_ms) VALUES (1, ?1) ON CONFLICT (id) DO UPDATE SET until_ms = excluded.until_ms",
        params![until_ms],
    )?;
    Ok(())
}

pub fn clear(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM claude_pause", [])?;
    Ok(())
}

/// The time Claude may be asked again, while that is still in the future.
pub fn active_until(connection: &Connection, now_ms: i64) -> rusqlite::Result<Option<i64>> {
    connection.query_row("SELECT until_ms FROM claude_pause WHERE until_ms > ?1", params![now_ms], |row| row.get(0)).optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    #[test]
    fn a_pause_lasts_until_its_time() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        assert_eq!(active_until(&connection, 1_000).unwrap(), None);
        set(&connection, 5_000).unwrap();
        set(&connection, 9_000).unwrap();
        assert_eq!(active_until(&connection, 1_000).unwrap(), Some(9_000));
        assert_eq!(active_until(&connection, 9_000).unwrap(), None);
        clear(&connection).unwrap();
        assert_eq!(active_until(&connection, 1_000).unwrap(), None);
    }
}
