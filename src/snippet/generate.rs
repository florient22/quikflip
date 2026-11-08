//! Picking Snippet's songs: one per chart source, mode and genre per day, from Deezer. No AI is involved.
//! A track is never used twice, and an artist rests for a month before coming back.
//!
//! Deezer's chart endpoints follow the country of whoever calls them, so the worldwide pools come from Deezer's own
//! "Top Worldwide" playlist and its genre editors' international playlists instead.

use anyhow::{Context, bail};
use chrono::{Days, NaiveDate};
use rand::seq::{IndexedRandom, SliceRandom};
use rusqlite::{Connection, OptionalExtension, params};

use super::deezer::{Deezer, Track};
use super::{MIX, RANDOM, SOURCE_SEPARATOR};
use crate::day::format_day;
use crate::settings::{ChartSource, Language, SnippetGenre};
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
/// Deezer Charts' "Top Worldwide" playlist: the worldwide daily mix.
const TOP_WORLDWIDE_PLAYLIST: u64 = 3_155_776_842;

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

/// Deezer editors' international playlists for each genre (hits plus a few eras, so the pool lasts).
/// If Deezer retires one, the refresh logs a warning and the others still fill the pool.
fn world_playlists(genre: SnippetGenre) -> &'static [u64] {
    match genre {
        SnippetGenre::Pop => &[1_964_085_082, 8_282_573_142, 8_326_097_522],
        SnippetGenre::Hiphop => &[1_677_006_641, 7_662_551_722, 12_547_421_383],
        SnippetGenre::Rock => &[752_286_631, 1_419_215_845, 8_621_268_482],
        SnippetGenre::Dance => &[706_093_725, 2_113_355_604, 1_495_242_491],
        SnippetGenre::Rnb => &[1_999_466_402, 2_021_626_162],
        SnippetGenre::Alternative => &[5_337_198_442, 11_335_739_484],
        SnippetGenre::Electro => &[1_902_101_402, 10_153_594_502],
        SnippetGenre::Metal => &[1_388_965_575, 2_655_390_504],
        SnippetGenre::Soul => &[1_257_789_321, 3_798_795_702, 1_840_772_682],
        SnippetGenre::Latin => &[178_699_142, 1_273_315_391],
        SnippetGenre::Chanson => &[700_895_155, 788_022_051],
        SnippetGenre::Dutch => &[1_352_999_805, 8_311_305_462, 8_311_306_722],
    }
}

/// The catalog pool for a source and a genre or the mix: "world/pop", "local/mix".
fn pool(source: ChartSource, choice: &str) -> String {
    format!("{}{SOURCE_SEPARATOR}{choice}", source.code())
}

pub fn has_song(connection: &Connection, day: &str, variant: &str) -> anyhow::Result<bool> {
    let count: i64 =
        connection.query_row("SELECT COUNT(*) FROM snippet_songs WHERE day = ?1 AND variant = ?2", params![day, variant], |row| row.get(0))?;
    Ok(count > 0)
}

fn add_to_catalog(connection: &mut Connection, tracks: &[Track], pool: &str) -> anyhow::Result<usize> {
    let transaction = connection.transaction()?;
    let mut added = 0;
    for track in tracks.iter().filter(|track| !track.preview.is_empty()) {
        let search = text::normalize(Language::En, &format!("{} {}", track.title_short, track.artist.name));
        added += transaction.execute(
            "INSERT INTO snippet_catalog (track_id, pool, title, artist, artist_id, rank, search) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (track_id, pool) DO UPDATE SET rank = excluded.rank",
            params![track.id as i64, pool, track.title_short, track.artist.name, track.artist.id as i64, track.rank, search],
        )?;
    }
    transaction.commit()?;
    Ok(added)
}

/// Adds one Deezer list to a pool. A failure is logged and skipped, so one retired playlist never blocks the rest.
fn fill(connection: &mut Connection, pool: &str, tracks: anyhow::Result<Vec<Track>>) -> usize {
    match tracks.and_then(|tracks| add_to_catalog(connection, &tracks, pool)) {
        Ok(added) => added,
        Err(error) => {
            tracing::warn!("Snippet: could not refresh {pool}: {error:#}");
            0
        }
    }
}

/// Fetches today's charts into the catalog: the local country charts and the worldwide playlists, for every genre.
pub async fn refresh_catalog(deezer: &Deezer, connection: &mut Connection) -> usize {
    let mut added = fill(connection, &pool(ChartSource::Local, MIX), deezer.chart(ALL_GENRES_CHART).await);
    added += fill(connection, &pool(ChartSource::World, MIX), deezer.playlist(TOP_WORLDWIDE_PLAYLIST).await);
    for genre in SnippetGenre::ALL {
        added += fill(connection, &pool(ChartSource::Local, genre.code()), deezer.chart(deezer_genre(genre)).await);
        for playlist in world_playlists(genre) {
            added += fill(connection, &pool(ChartSource::World, genre.code()), deezer.playlist(*playlist).await);
        }
    }
    added
}

/// Adds the top tracks of some of the genre's popular artists, for when its pool has been used up.
async fn widen_catalog(deezer: &Deezer, connection: &mut Connection, pool: &str, genre: SnippetGenre) -> anyhow::Result<()> {
    let mut artists = deezer.genre_artists(deezer_genre(genre)).await?;
    artists.shuffle(&mut rand::rng());
    for artist in artists.iter().take(ARTISTS_TO_WIDEN) {
        add_to_catalog(connection, &deezer.artist_top(artist.id).await?, pool)?;
    }
    Ok(())
}

/// Popular tracks never used before, from artists not used within the cooldown (around this day, before or after).
/// `pools` is a LIKE pattern: one pool ("world/pop") or all of a source's pools ("world/%") for the daily mix.
fn eligible(connection: &Connection, pools: &str, day: NaiveDate) -> anyhow::Result<Vec<u64>> {
    let since = format_day(day - Days::new(ARTIST_COOLDOWN_DAYS));
    let mut statement = connection.prepare(
        "SELECT track_id FROM snippet_catalog
         WHERE pool LIKE ?1
           AND track_id NOT IN (SELECT track_id FROM snippet_songs)
           AND artist_id NOT IN (SELECT artist_id FROM snippet_songs WHERE day >= ?2)
         GROUP BY track_id ORDER BY MAX(rank) DESC LIMIT ?3",
    )?;
    let tracks = statement
        .query_map(params![pools, since, POOL_TOP], |row| row.get::<_, i64>(0))?
        .map(|id| id.map(|id| id as u64))
        .collect::<Result<_, _>>()?;
    Ok(tracks)
}

/// The genre for the random mode: any genre except the one it had the day before (per chart source).
fn random_genre(connection: &Connection, source: ChartSource, day: NaiveDate) -> anyhow::Result<SnippetGenre> {
    let yesterday: Option<String> = connection
        .query_row(
            "SELECT genre FROM snippet_songs WHERE day = ?1 AND variant = ?2",
            params![format_day(day - Days::new(1)), pool(source, RANDOM)],
            |row| row.get(0),
        )
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
    let cover = match detail.album.cover_medium.as_deref().filter(|url| !url.is_empty()) {
        Some(url) => deezer.download(url).await.ok(),
        None => None,
    };
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO snippet_songs (day, variant, genre, track_id, title, artist, artist_id, album, link) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![day, variant, genre, detail.id as i64, detail.title_short, detail.artist.name, detail.artist.id as i64, detail.album.title, detail.link],
    )?;
    transaction.execute("INSERT INTO snippet_audio (day, variant, mp3, cover) VALUES (?1, ?2, ?3, ?4)", params![day, variant, mp3, cover])?;
    transaction.commit()?;
    Ok(())
}

/// Picks and stores the song for one day and variant ("world/mix", "local/random", "world/metal"…).
pub async fn generate(deezer: &Deezer, connection: &mut Connection, day: NaiveDate, variant: &str) -> anyhow::Result<()> {
    let day_text = format_day(day);
    if has_song(connection, &day_text, variant)? {
        return Ok(());
    }
    let (source, choice) = variant
        .split_once(SOURCE_SEPARATOR)
        .and_then(|(source, choice)| Some((ChartSource::from_code(source)?, choice)))
        .with_context(|| format!("unknown Snippet variant {variant:?}"))?;
    let genre = match choice {
        MIX => None,
        RANDOM => Some(random_genre(connection, source, day)?),
        code => Some(SnippetGenre::from_code(code).with_context(|| format!("unknown Snippet variant {variant:?}"))?),
    };
    let pools = genre.map_or_else(|| pool(source, "%"), |genre| pool(source, genre.code()));
    let mut candidates = eligible(connection, &pools, day)?;
    if let Some(genre) = genre
        && candidates.len() < MIN_POOL
    {
        widen_catalog(deezer, connection, &pools, genre).await?;
        candidates = eligible(connection, &pools, day)?;
    }
    candidates.shuffle(&mut rand::rng());
    let genre_code = genre.map_or(MIX, SnippetGenre::code);
    let mut last_error = None;
    for track_id in candidates.into_iter().take(CANDIDATES_TO_TRY) {
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
        add_to_catalog(&mut connection, &[track(1, 10, 900), track(2, 10, 800), track(3, 20, 700)], "world/pop").unwrap();
        // The same track in two pools is one candidate.
        add_to_catalog(&mut connection, &[track(3, 20, 700)], "world/mix").unwrap();
        connection.execute("INSERT INTO snippet_songs VALUES ('2026-11-01', 'world/pop', 'pop', 1, 'Song 1', 'Artist 10', 10, 'Album', 'link')", []).unwrap();
        let day = crate::day::parse_day("2026-11-08").unwrap();
        assert_eq!(eligible(&connection, "world/pop", day).unwrap(), vec![3]);
        assert!(eligible(&connection, "world/rock", day).unwrap().is_empty());
        assert!(eligible(&connection, "local/%", day).unwrap().is_empty());
        // A month and a day later the artist may come back, but never the same track.
        let later = crate::day::parse_day("2026-12-02").unwrap();
        assert_eq!(eligible(&connection, "world/%", later).unwrap(), vec![2, 3]);
    }
}
