use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Bump when a settings shape changes incompatibly; older rows then fall back to defaults.
const SETTINGS_VERSION: i64 = 1;
const APP_SCOPE: &str = "app";
const LETTERLOCK_SCOPE: &str = "letterlock";
const RAREFIND_SCOPE: &str = "rarefind";
const ATLAS_SCOPE: &str = "atlas";
const SNIPPET_SCOPE: &str = "snippet";

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

/// Which part of the world Atlas Hunt's clues cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AtlasRegion {
    #[default]
    World,
    Europe,
    Africa,
    Americas,
    Asia,
    Oceania,
}

impl AtlasRegion {
    pub const ALL: [AtlasRegion; 6] =
        [AtlasRegion::World, AtlasRegion::Europe, AtlasRegion::Africa, AtlasRegion::Americas, AtlasRegion::Asia, AtlasRegion::Oceania];

    pub fn code(self) -> &'static str {
        match self {
            AtlasRegion::World => "world",
            AtlasRegion::Europe => "europe",
            AtlasRegion::Africa => "africa",
            AtlasRegion::Americas => "americas",
            AtlasRegion::Asia => "asia",
            AtlasRegion::Oceania => "oceania",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|region| region.code() == code)
    }

    /// The region's name in the country data, or None for the whole world.
    pub fn dataset_name(self) -> Option<&'static str> {
        match self {
            AtlasRegion::World => None,
            AtlasRegion::Europe => Some("Europe"),
            AtlasRegion::Africa => Some("Africa"),
            AtlasRegion::Americas => Some("Americas"),
            AtlasRegion::Asia => Some("Asia"),
            AtlasRegion::Oceania => Some("Oceania"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AtlasSettings {
    pub region: AtlasRegion,
    /// Show a fun fact about every country you find.
    pub facts: bool,
}

impl Default for AtlasSettings {
    fn default() -> Self {
        Self { region: AtlasRegion::World, facts: true }
    }
}

/// Where Snippet's daily song comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnippetMode {
    /// A popular song from any genre.
    #[default]
    Mix,
    /// A different genre every day, shown as a hint.
    Random,
    /// Always the genre picked in the settings.
    Genre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnippetGenre {
    #[default]
    Pop,
    Hiphop,
    Rock,
    Dance,
    Rnb,
    Alternative,
    Electro,
    Metal,
    Soul,
    Latin,
    Chanson,
    Dutch,
}

impl SnippetGenre {
    pub const ALL: [SnippetGenre; 12] = [
        SnippetGenre::Pop,
        SnippetGenre::Hiphop,
        SnippetGenre::Rock,
        SnippetGenre::Dance,
        SnippetGenre::Rnb,
        SnippetGenre::Alternative,
        SnippetGenre::Electro,
        SnippetGenre::Metal,
        SnippetGenre::Soul,
        SnippetGenre::Latin,
        SnippetGenre::Chanson,
        SnippetGenre::Dutch,
    ];

    pub fn code(self) -> &'static str {
        match self {
            SnippetGenre::Pop => "pop",
            SnippetGenre::Hiphop => "hiphop",
            SnippetGenre::Rock => "rock",
            SnippetGenre::Dance => "dance",
            SnippetGenre::Rnb => "rnb",
            SnippetGenre::Alternative => "alternative",
            SnippetGenre::Electro => "electro",
            SnippetGenre::Metal => "metal",
            SnippetGenre::Soul => "soul",
            SnippetGenre::Latin => "latin",
            SnippetGenre::Chanson => "chanson",
            SnippetGenre::Dutch => "dutch",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|genre| genre.code() == code)
    }
}

/// How long Snippet's clips are, from the first clip to the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipLength {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl ClipLength {
    /// Seconds of the song unlocked at each stage; a skip or a wrong guess moves to the next stage.
    pub fn seconds(self) -> [f64; 6] {
        match self {
            ClipLength::Easy => [2.0, 4.0, 7.0, 11.0, 16.0, 24.0],
            ClipLength::Normal => [1.0, 2.0, 4.0, 7.0, 11.0, 16.0],
            ClipLength::Hard => [0.5, 1.0, 2.0, 4.0, 8.0, 15.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SnippetSettings {
    pub mode: SnippetMode,
    /// Used when the mode is Genre.
    pub genre: SnippetGenre,
    pub clips: ClipLength,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllSettings {
    pub app: AppSettings,
    pub letterlock: LetterlockSettings,
    pub rarefind: RarefindSettings,
    pub atlas: AtlasSettings,
    pub snippet: SnippetSettings,
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

pub fn atlas(connection: &Connection) -> Result<AtlasSettings, AppError> {
    load(connection, ATLAS_SCOPE)
}

pub fn snippet(connection: &Connection) -> Result<SnippetSettings, AppError> {
    load(connection, SNIPPET_SCOPE)
}

pub fn all(connection: &Connection) -> Result<AllSettings, AppError> {
    Ok(AllSettings {
        app: app(connection)?,
        letterlock: letterlock(connection)?,
        rarefind: rarefind(connection)?,
        atlas: atlas(connection)?,
        snippet: snippet(connection)?,
    })
}

pub fn save_atlas(connection: &Connection, value: &AtlasSettings) -> Result<(), AppError> {
    save(connection, ATLAS_SCOPE, value)
}

pub fn save_snippet(connection: &Connection, value: &SnippetSettings) -> Result<(), AppError> {
    save(connection, SNIPPET_SCOPE, value)
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

    #[test]
    fn missing_fields_keep_their_defaults() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        connection.execute(r#"INSERT INTO settings VALUES ('atlas', 1, '{"region":"asia"}')"#, []).unwrap();
        assert_eq!(atlas(&connection).unwrap(), AtlasSettings { region: AtlasRegion::Asia, facts: true });
        connection.execute(r#"INSERT INTO settings VALUES ('snippet', 1, '{"mode":"genre","genre":"metal"}')"#, []).unwrap();
        let loaded = snippet(&connection).unwrap();
        assert_eq!((loaded.mode, loaded.genre, loaded.clips), (SnippetMode::Genre, SnippetGenre::Metal, ClipLength::Normal));
    }
}
