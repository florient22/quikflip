//! The free Deezer API (no account or key): genre charts, artists' top tracks, track details and 30-second previews.
//! Only the generator calls it; the web server serves what the generator stored.

use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const API: &str = "https://api.deezer.com";
const TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("QuikFlip/", env!("CARGO_PKG_VERSION"), " (self-hosted daily games)");
const CHART_LIMIT: usize = 100;
const TOP_LIMIT: usize = 25;
const PLAYLIST_LIMIT: usize = 300;
/// Previews are about 0.5 MB and covers about 20 KB; anything far bigger is not what we asked for.
const MAX_DOWNLOAD_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub id: u64,
    /// The title without "(feat. …)" or version notes.
    pub title_short: String,
    pub artist: ArtistRef,
    #[serde(default)]
    pub rank: i64,
    #[serde(default)]
    pub preview: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub title: String,
    /// Missing (null) for albums without artwork.
    #[serde(default)]
    pub cover_medium: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackDetail {
    pub id: u64,
    pub title_short: String,
    pub link: String,
    pub preview: String,
    pub artist: ArtistRef,
    pub album: Album,
}

#[derive(Debug, Deserialize)]
struct List<T> {
    data: Vec<T>,
}

pub struct Deezer {
    client: reqwest::Client,
}

impl Deezer {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder().timeout(TIMEOUT).user_agent(USER_AGENT).build().context("could not set up the HTTP client")?;
        Ok(Self { client })
    }

    /// GETs an API path. Deezer reports errors as `{"error": …}` with status 200, so both are checked.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{API}{path}");
        let value: Value = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("could not reach Deezer ({url})"))?
            .error_for_status()
            .with_context(|| format!("Deezer refused {url}"))?
            .json()
            .await
            .with_context(|| format!("Deezer sent something that is not JSON for {url}"))?;
        if let Some(error) = value.get("error") {
            bail!("Deezer returned an error for {url}: {error}");
        }
        serde_json::from_value(value).with_context(|| format!("Deezer's answer for {url} had an unexpected shape"))
    }

    /// The most popular tracks in a genre (0 = all genres).
    pub async fn chart(&self, genre_id: u32) -> anyhow::Result<Vec<Track>> {
        Ok(self.get::<List<Track>>(&format!("/chart/{genre_id}/tracks?limit={CHART_LIMIT}")).await?.data)
    }

    /// A playlist's tracks (Deezer's charts and editors' playlists are at most a few hundred long).
    pub async fn playlist(&self, playlist_id: u64) -> anyhow::Result<Vec<Track>> {
        Ok(self.get::<List<Track>>(&format!("/playlist/{playlist_id}/tracks?limit={PLAYLIST_LIMIT}")).await?.data)
    }

    pub async fn genre_artists(&self, genre_id: u32) -> anyhow::Result<Vec<ArtistRef>> {
        Ok(self.get::<List<ArtistRef>>(&format!("/genre/{genre_id}/artists")).await?.data)
    }

    pub async fn artist_top(&self, artist_id: u64) -> anyhow::Result<Vec<Track>> {
        Ok(self.get::<List<Track>>(&format!("/artist/{artist_id}/top?limit={TOP_LIMIT}")).await?.data)
    }

    /// Full details, including a fresh preview link (preview links expire after a while).
    pub async fn track(&self, track_id: u64) -> anyhow::Result<TrackDetail> {
        self.get(&format!("/track/{track_id}")).await
    }

    pub async fn download(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("could not download from Deezer")?
            .error_for_status()
            .context("Deezer refused the download")?;
        if response.content_length().is_some_and(|length| length as usize > MAX_DOWNLOAD_BYTES) {
            bail!("the download from Deezer is larger than {MAX_DOWNLOAD_BYTES} bytes");
        }
        let bytes = response.bytes().await.context("the download from Deezer was interrupted")?;
        if bytes.len() > MAX_DOWNLOAD_BYTES {
            bail!("the download from Deezer is larger than {MAX_DOWNLOAD_BYTES} bytes");
        }
        Ok(bytes.to_vec())
    }
}
