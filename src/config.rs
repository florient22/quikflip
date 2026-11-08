use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono_tz::Tz;

const DEFAULT_BIND: &str = "0.0.0.0:8080";
const DEFAULT_TIMEZONE: &str = "Europe/Brussels";
const DATABASE_FILE: &str = "quikflip.sqlite3";

/// Runtime configuration, read from environment variables. Paths never depend on the working directory.
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    pub bind: SocketAddr,
    pub timezone: Tz,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let exe_dir = env::current_exe()
            .context("could not find the QuikFlip executable's location")?
            .parent()
            .map(Path::to_path_buf)
            .context("the QuikFlip executable has no parent folder")?;

        let data_dir = env::var_os("QUIKFLIP_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| exe_dir.join("data"));
        let web_dir = match env::var_os("QUIKFLIP_WEB_DIR") {
            Some(path) => PathBuf::from(path),
            // Next to the binary in the Docker image; the source tree during development.
            None if exe_dir.join("web").is_dir() => exe_dir.join("web"),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).join("web"),
        };
        let bind_text = env::var("QUIKFLIP_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
        let bind = bind_text
            .parse()
            .with_context(|| format!("QUIKFLIP_BIND must look like 0.0.0.0:8080, got {bind_text:?}"))?;

        let timezone_text = env::var("QUIKFLIP_TIMEZONE").unwrap_or_else(|_| DEFAULT_TIMEZONE.to_owned());
        let timezone = timezone_text
            .parse::<Tz>()
            .map_err(|_| anyhow::anyhow!("QUIKFLIP_TIMEZONE must be a time zone like Europe/Brussels, got {timezone_text:?}"))?;

        Ok(Self { data_dir, web_dir, bind, timezone })
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join(DATABASE_FILE)
    }

    /// Only the web server needs the front end; the generator runs without it.
    pub fn require_web(&self) -> anyhow::Result<()> {
        if !self.web_dir.join("index.html").is_file() {
            bail!("no web/index.html found in {}; set QUIKFLIP_WEB_DIR to the folder with the front end", self.web_dir.display());
        }
        Ok(())
    }
}
