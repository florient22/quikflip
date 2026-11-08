use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Bump when a settings shape changes incompatibly; older rows then fall back to defaults.
const SETTINGS_VERSION: i64 = 1;
const APP_SCOPE: &str = "app";
const LETTERLOCK_SCOPE: &str = "letterlock";
const RAREFIND_SCOPE: &str = "rarefind";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Fr,
    Nl,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::En, Language::Fr, Language::Nl];

    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Fr => "fr",
            Language::Nl => "nl",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.code() == code)
    }

    /// French and Dutch (Belgian) players type on AZERTY keyboards.
    pub fn keyboard(self) -> &'static str {
        match self {
            Language::En => "qwerty",
            Language::Fr | Language::Nl => "azerty",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub language: Language,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InkSet {
    #[default]
    Tangerine,
    Forest,
    Newsprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LetterlockSettings {
    pub ink_set: InkSet,
    pub hard_mode: bool,
}

/// How long each Rarefind prompt may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RarefindTimer {
    #[default]
    Normal,
    Relaxed,
    Off,
}

impl RarefindTimer {
    pub fn seconds(self) -> Option<i64> {
        match self {
            RarefindTimer::Normal => Some(25),
            RarefindTimer::Relaxed => Some(45),
            RarefindTimer::Off => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RarefindSettings {
    pub timer: RarefindTimer,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllSettings {
    pub app: AppSettings,
    pub letterlock: LetterlockSettings,
    pub rarefind: RarefindSettings,
}

fn load<T: DeserializeOwned + Default>(connection: &Connection, scope: &str) -> Result<T, AppError> {
    let stored: Option<(i64, String)> = connection
        .query_row("SELECT version, value FROM settings WHERE scope = ?1", params![scope], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let Some((version, value)) = stored else { return Ok(T::default()) };
    if version != SETTINGS_VERSION {
        tracing::warn!("settings for {scope} have version {version}, expected {SETTINGS_VERSION}; using defaults");
        return Ok(T::default());
    }
    Ok(serde_json::from_str(&value).unwrap_or_else(|error| {
        tracing::warn!("settings for {scope} could not be read ({error}); using defaults");
        T::default()
    }))
}

fn save<T: Serialize>(connection: &Connection, scope: &str, value: &T) -> Result<(), AppError> {
    let json = serde_json::to_string(value).map_err(|error| AppError::Corrupt(error.to_string()))?;
    connection.execute(
        "INSERT INTO settings (scope, version, value) VALUES (?1, ?2, ?3)
         ON CONFLICT (scope) DO UPDATE SET version = excluded.version, value = excluded.value",
        params![scope, SETTINGS_VERSION, json],
    )?;
    Ok(())
}

pub fn app(connection: &Connection) -> Result<AppSettings, AppError> {
    load(connection, APP_SCOPE)
}

pub fn letterlock(connection: &Connection) -> Result<LetterlockSettings, AppError> {
    load(connection, LETTERLOCK_SCOPE)
}

pub fn rarefind(connection: &Connection) -> Result<RarefindSettings, AppError> {
    load(connection, RAREFIND_SCOPE)
}

pub fn all(connection: &Connection) -> Result<AllSettings, AppError> {
    Ok(AllSettings { app: app(connection)?, letterlock: letterlock(connection)?, rarefind: rarefind(connection)? })
}

pub fn save_rarefind(connection: &Connection, value: &RarefindSettings) -> Result<(), AppError> {
    save(connection, RAREFIND_SCOPE, value)
}

pub fn save_app(connection: &Connection, value: &AppSettings) -> Result<(), AppError> {
    save(connection, APP_SCOPE, value)
}

pub fn save_letterlock(connection: &Connection, value: &LetterlockSettings) -> Result<(), AppError> {
    save(connection, LETTERLOCK_SCOPE, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    #[test]
    fn missing_or_unreadable_settings_fall_back_to_defaults() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        assert_eq!(app(&connection).unwrap(), AppSettings::default());
        connection.execute("INSERT INTO settings VALUES ('app', 1, 'not json')", []).unwrap();
        assert_eq!(app(&connection).unwrap(), AppSettings::default());
        save_app(&connection, &AppSettings { language: Language::Nl }).unwrap();
        assert_eq!(app(&connection).unwrap().language, Language::Nl);
    }
}
