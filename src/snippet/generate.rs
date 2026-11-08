//! Picking Snippet's songs: one per mode and genre per day, from Deezer's charts. No AI is involved.
//! A track is never used twice, and an artist rests for a month before coming back.

use anyhow::{Context, bail};
use chrono::{Days, NaiveDate};
use rand::seq::{IndexedRandom, SliceRandom};
use rusqlite::{Connection, OptionalExtension, params};

use super::deezer::{Deezer, Track};
use super::{MIX, RANDOM};
use crate::day::format_day;
use crate::settings::{Language, SnippetGenre};
use crate::text;

const ARTIST_COOLDOWN_DAYS: u64 = 30;
/// The day's song is drawn from this many of the most popular eligible tracks.
const POOL_TOP: i64 = 40;
/// When fewer tracks than this are eligible, the genre's popular artists' top tracks are added to the catalog.
const MIN_POOL: usize = 15;
const ARTISTS_TO_WIDEN: usize = 8;
const CANDIDATES_TO_TRY: usize = 3;
const MIN_PREVIEW_BYTES: usize = 100_000;
/// Audio is kept until the day after its puzzle, then deleted (the song details stay for the history).
const AUDIO_KEEP_DAYS: u64 = 1;
const ALL_GENRES_CHART: u32 = 0;

fn deezer_genre(genre: SnippetGenre) -> u32 {
    match genre {
        SnippetGenre::Pop => 132,
        SnippetGenre::Hiphop => 116,
        SnippetGenre::Rock => 152,
        SnippetGenre::Dance => 113,
        SnippetGenre::Rnb => 165,
        SnippetGenre::Alternative => 85,
        SnippetGenre::Electro => 106,
        SnippetGenre::Metal => 464,
        SnippetGenre::Soul => 169,
        SnippetGenre::Latin => 197,
        SnippetGenre::Chanson => 52,
        SnippetGenre::Dutch => 518,
    }
}

pub fn has_song(connection: &Connection, day: &str, variant: &str) -> anyhow::Result<bool> {
    let count: i64 =
        connection.query_row("SELECT COUNT(*) FROM snippet_songs WHERE day = ?1 AND variant = ?2", params![day, variant], |row| row.get(0))?;
    Ok(count > 0)
}

fn add_to_catalog(connection: &mut Connection, tracks: &[Track], genre: &str) -> anyhow::Result<usize> {
    let transaction = connection.transaction()?;
    let mut added = 0;
    for track in tracks.iter().filter(|track| !track.preview.is_empty()) {
        let search = text::normalize(Language::En, &format!("{} {}", track.title_short, track.artist.name));
        added += transaction.execute(
            "INSERT INTO snippet_catalog (track_id, title, artist, artist_id, genre, rank, search) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (track_id) DO UPDATE SET rank = excluded.rank",
            params![track.id as i64, track.title_short, track.artist.name, track.artist.id as i64, genre, track.rank, search],
        )?;
    }
    transaction.commit()?;
    Ok(added)
}

/// Fetches today's charts for every genre (and the all-genre chart for the daily mix) into the catalog.
pub async fn refresh_catalog(deezer: &Deezer, connection: &mut Connection) -> anyhow::Result<usize> {
    let mut added = add_to_catalog(connection, &deezer.chart(ALL_GENRES_CHART).await?, MIX)?;
    for genre in SnippetGenre::ALL {
        added += add_to_catalog(connection, &deezer.chart(deezer_genre(genre)).await?, genre.code())?;
    }
    Ok(added)
}

/// Adds the top tracks of some of the genre's popular artists, for when its chart has been used up.
async fn widen_catalog(deezer: &Deezer, connection: &mut Connection, genre: SnippetGenre) -> anyhow::Result<()> {
    let mut artists = deezer.genre_artists(deezer_genre(genre)).await?;
    artists.shuffle(&mut rand::rng());
    for artist in artists.iter().take(ARTISTS_TO_WIDEN) {
        add_to_catalog(connection, &deezer.artist_top(artist.id).await?, genre.code())?;
    }
    Ok(())
}

/// Popular tracks never used before, from artists not used within the cooldown (around this day, before or after).
fn eligible(connection: &Connection, genre: Option<SnippetGenre>, day: NaiveDate) -> anyhow::Result<Vec<u64>> {
    let since = format_day(day - Days::new(ARTIST_COOLDOWN_DAYS));
    let mut statement = connection.prepare(
        "SELECT track_id FROM snippet_catalog
         WHERE (?1 IS NULL OR genre = ?1)
           AND track_id NOT IN (SELECT track_id FROM snippet_songs)
           AND artist_id NOT IN (SELECT artist_id FROM snippet_songs WHERE day >= ?2)
         ORDER BY rank DESC LIMIT ?3",
    )?;
    let tracks = statement
        .query_map(params![genre.map(SnippetGenre::code), since, POOL_TOP], |row| row.get::<_, i64>(0))?
        .map(|id| id.map(|id| id as u64))
        .collect::<Result<_, _>>()?;
    Ok(tracks)
}

/// The genre for the random mode: any genre except the one it had the day before.
fn random_genre(connection: &Connection, day: NaiveDate) -> anyhow::Result<SnippetGenre> {
    let yesterday: Option<String> = connection
        .query_row("SELECT genre FROM snippet_songs WHERE day = ?1 AND variant = ?2", params![format_day(day - Days::new(1)), RANDOM], |row| row.get(0))
        .optional()?;
    let options: Vec<SnippetGenre> = SnippetGenre::ALL.into_iter().filter(|genre| Some(genre.code()) != yesterday.as_deref()).collect();
    options.choose(&mut rand::rng()).copied().context("there are no genres to choose from")
}

fn looks_like_mp3(data: &[u8]) -> bool {
    data.len() >= MIN_PREVIEW_BYTES && (data.starts_with(b"ID3") || (data[0] == 0xFF && data[1] & 0xE0 == 0xE0))
}

/// Downloads the track's preview and cover and stores the song for the day.
async fn store_song(deezer: &Deezer, connection: &mut Connection, day: &str, variant: &str, genre: &str, track_id: u64) -> anyhow::Result<()> {
    let detail = deezer.track(track_id).await?;
    if detail.preview.is_empty() {
        bail!("track {track_id} has no preview");
    }
    let mp3 = deezer.download(&detail.preview).await?;
    if !looks_like_mp3(&mp3) {
        bail!("the preview of track {track_id} is not a usable MP3 ({} bytes)", mp3.len());
    }
    // The cover is a bonus at the end; a missing one is fine.
    let cover = if detail.album.cover_medium.is_empty() { None } else { deezer.download(&detail.album.cover_medium).await.ok() };
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO snippet_songs (day, variant, genre, track_id, title, artist, artist_id, album, link) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![day, variant, genre, detail.id as i64, detail.title_short, detail.artist.name, detail.artist.id as i64, detail.album.title, detail.link],
    )?;
    transaction.execute("INSERT INTO snippet_audio (day, variant, mp3, cover) VALUES (?1, ?2, ?3, ?4)", params![day, variant, mp3, cover])?;
    transaction.commit()?;
    Ok(())
}

/// Picks and stores the song for one day and variant ("mix", "random" or a genre code).
pub async fn generate(deezer: &Deezer, connection: &mut Connection, day: NaiveDate, variant: &str) -> anyhow::Result<()> {
    let day_text = format_day(day);
    if has_song(connection, &day_text, variant)? {
        return Ok(());
    }
    let genre = match variant {
        MIX => None,
        RANDOM => Some(random_genre(connection, day)?),
        code => Some(SnippetGenre::from_code(code).with_context(|| format!("unknown Snippet variant {code:?}"))?),
    };
    let mut pool = eligible(connection, genre, day)?;
    if let Some(genre) = genre
        && pool.len() < MIN_POOL
    {
        widen_catalog(deezer, connection, genre).await?;
        pool = eligible(connection, genre.into(), day)?;
    }
    pool.shuffle(&mut rand::rng());
    let genre_code = genre.map_or(MIX, SnippetGenre::code);
    let mut last_error = None;
    for track_id in pool.into_iter().take(CANDIDATES_TO_TRY) {
        match store_song(deezer, connection, &day_text, variant, genre_code, track_id).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!("Snippet {day_text} {variant}: track {track_id} skipped: {error:#}");
                last_error = Some(error);
            }
        }
    }
    match last_error {
        Some(error) => Err(error.context("no usable track")),
        None => bail!("no eligible tracks left in the catalog for {variant}"),
    }
}

/// Deletes audio whose day is over; the song details stay so tracks and artists are never repeated too soon.
pub fn delete_old_audio(connection: &Connection, today: NaiveDate) -> anyhow::Result<usize> {
    let before = format_day(today - Days::new(AUDIO_KEEP_DAYS));
    Ok(connection.execute("DELETE FROM snippet_audio WHERE day < ?1", params![before])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    fn track(id: u64, artist: u64, rank: i64) -> Track {
        Track {
            id,
            title_short: format!("Song {id}"),
            artist: super::super::deezer::ArtistRef { id: artist, name: format!("Artist {artist}") },
            rank,
            preview: "https://example.invalid/preview.mp3".into(),
        }
    }

    #[test]
    fn used_tracks_and_resting_artists_are_not_eligible() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        add_to_catalog(&mut connection, &[track(1, 10, 900), track(2, 10, 800), track(3, 20, 700)], "pop").unwrap();
        connection.execute("INSERT INTO snippet_songs VALUES ('2026-11-01', 'pop', 'pop', 1, 'Song 1', 'Artist 10', 10, 'Album', 'link')", []).unwrap();
        let day = crate::day::parse_day("2026-11-08").unwrap();
        assert_eq!(eligible(&connection, Some(SnippetGenre::Pop), day).unwrap(), vec![3]);
        assert!(eligible(&connection, Some(SnippetGenre::Rock), day).unwrap().is_empty());
        // A month and a day later the artist may come back, but never the same track.
        let later = crate::day::parse_day("2026-12-02").unwrap();
        assert_eq!(eligible(&connection, None, later).unwrap(), vec![2, 3]);
    }
}
