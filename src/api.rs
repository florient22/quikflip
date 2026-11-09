use std::path::Path;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

use crate::day::{Clock, format_day};
use crate::db::Db;
use crate::error::AppError;
use crate::settings::{self, AllSettings, AppSettings, AtlasSettings, Language, LetterlockSettings, RarefindSettings, SnippetSettings};
use crate::{atlas, claude_pause, letterlock, rarefind, snippet};

/// Clips and covers depend on how far the player is, so the browser must never reuse an old copy.
const NO_STORE: &str = "no-store";
const MP3_TYPE: &str = "audio/mpeg";
/// Deezer album covers are JPEG.
const COVER_TYPE: &str = "image/jpeg";

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub clock: Clock,
}

pub fn router(state: AppState, web_dir: &Path) -> Router {
    Router::new()
        .route("/api/home", get(home))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/app", put(put_app_settings))
        .route("/api/settings/letterlock", put(put_letterlock_settings))
        .route("/api/settings/rarefind", put(put_rarefind_settings))
        .route("/api/letterlock", get(get_letterlock))
        .route("/api/letterlock/guess", post(post_guess))
        .route("/api/rarefind", get(get_rarefind))
        .route("/api/rarefind/start", post(post_rarefind_start))
        .route("/api/rarefind/answer", post(post_rarefind_answer))
        .route("/api/rarefind/expire", post(post_rarefind_expire))
        .route("/api/rarefind/contest", post(post_rarefind_contest))
        .route("/api/rarefind/autofill", get(get_rarefind_autofill))
        .route("/api/settings/atlas", put(put_atlas_settings))
        .route("/api/atlas", get(get_atlas))
        .route("/api/atlas/pick", post(post_atlas_pick))
        .route("/api/settings/snippet", put(put_snippet_settings))
        .route("/api/snippet", get(get_snippet))
        .route("/api/snippet/guess", post(post_snippet_guess))
        .route("/api/snippet/skip", post(post_snippet_skip))
        .route("/api/snippet/clip", get(get_snippet_clip))
        .route("/api/snippet/cover", get(get_snippet_cover))
        .route("/api/snippet/suggest", get(get_snippet_suggest))
        .fallback_service(ServeDir::new(web_dir).append_index_html_on_directories(true))
        .with_state(state)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// One tile on the homepage. Each game fills the picture field it uses.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameSummary {
    id: &'static str,
    status: &'static str,
    used: usize,
    max: usize,
    streak: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    marks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tiers: Option<Vec<Option<u8>>>,
}

fn letterlock_status(status: letterlock::Status) -> &'static str {
    match status {
        letterlock::Status::New => "new",
        letterlock::Status::Progress => "progress",
        letterlock::Status::Solved => "solved",
        letterlock::Status::Failed => "failed",
    }
}

fn atlas_status(status: atlas::Status) -> &'static str {
    match status {
        atlas::Status::Waiting => "waiting",
        atlas::Status::New => "new",
        atlas::Status::Progress => "progress",
        atlas::Status::Solved => "solved",
    }
}

fn snippet_status(status: snippet::Status) -> &'static str {
    match status {
        snippet::Status::Waiting => "waiting",
        snippet::Status::New => "new",
        snippet::Status::Progress => "progress",
        snippet::Status::Solved => "solved",
        snippet::Status::Failed => "failed",
    }
}

fn rarefind_status(status: rarefind::Status) -> &'static str {
    match status {
        rarefind::Status::Waiting => "waiting",
        rarefind::Status::New => "new",
        rarefind::Status::Progress => "progress",
        rarefind::Status::Solved => "solved",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeSummary {
    date: String,
    seconds_to_reset: i64,
    language: Language,
    games: Vec<GameSummary>,
    /// While Claude's usage limit holds new Rarefind and Atlas Hunt puzzles back: when it resets (Unix ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    claude_paused_until: Option<i64>,
}

async fn home(State(state): State<AppState>) -> Result<Json<HomeSummary>, AppError> {
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let app = settings::app(connection)?;
            let lock = letterlock::state(connection, today, app.language, &settings::letterlock(connection)?)?;
            let find = rarefind::state(connection, today, app.language, &settings::rarefind(connection)?, now_ms())?;
            let finished_prompts = find.prompts.iter().filter(|prompt| matches!(prompt.outcome, rarefind::Outcome::Answered | rarefind::Outcome::TimedOut)).count();
            let hunt = atlas::state(connection, today, app.language, &settings::atlas(connection)?)?;
            let finished_clues = hunt.clues.iter().filter(|clue| matches!(clue.outcome, atlas::Outcome::Found | atlas::Outcome::Lost)).count();
            let song = snippet::state(connection, today, &settings::snippet(connection)?)?;
            Ok(Json(HomeSummary {
                date: format_day(today),
                seconds_to_reset,
                language: app.language,
                claude_paused_until: claude_pause::active_until(connection, now_ms())?,
                games: vec![
                    GameSummary {
                        id: "letterlock",
                        status: letterlock_status(lock.status),
                        used: lock.guesses.len(),
                        max: lock.max_guesses,
                        streak: lock.streak,
                        marks: Some(lock.guesses.into_iter().map(|guess| guess.marks).collect()),
                        tiers: None,
                    },
                    GameSummary {
                        id: "rarefind",
                        status: rarefind_status(find.status),
                        used: finished_prompts,
                        max: rarefind::PROMPT_COUNT,
                        streak: find.streak,
                        marks: None,
                        tiers: Some(rarefind::tile_tiers(&find)),
                    },
                    GameSummary {
                        id: "atlas",
                        status: atlas_status(hunt.status),
                        used: finished_clues,
                        max: atlas::CLUE_COUNT,
                        streak: hunt.streak,
                        marks: None,
                        tiers: Some(atlas::tile_tiers(&hunt)),
                    },
                    GameSummary {
                        id: "snippet",
                        status: snippet_status(song.status),
                        used: song.guesses.len(),
                        max: song.max_guesses,
                        streak: song.streak,
                        marks: Some(snippet::tile_marks(&song)),
                        tiers: None,
                    },
                ],
            }))
        })
        .await
}

async fn get_settings(State(state): State<AppState>) -> Result<Json<AllSettings>, AppError> {
    state.db.call(|connection| Ok(Json(settings::all(connection)?))).await
}

async fn put_app_settings(State(state): State<AppState>, Json(value): Json<AppSettings>) -> Result<Json<AllSettings>, AppError> {
    state
        .db
        .call(move |connection| {
            settings::save_app(connection, &value)?;
            Ok(Json(settings::all(connection)?))
        })
        .await
}

async fn put_letterlock_settings(
    State(state): State<AppState>,
    Json(value): Json<LetterlockSettings>,
) -> Result<Json<AllSettings>, AppError> {
    state
        .db
        .call(move |connection| {
            settings::save_letterlock(connection, &value)?;
            Ok(Json(settings::all(connection)?))
        })
        .await
}

async fn put_rarefind_settings(State(state): State<AppState>, Json(value): Json<RarefindSettings>) -> Result<Json<AllSettings>, AppError> {
    state
        .db
        .call(move |connection| {
            settings::save_rarefind(connection, &value)?;
            Ok(Json(settings::all(connection)?))
        })
        .await
}

#[derive(Deserialize)]
struct LanguageQuery {
    lang: Option<String>,
}

/// The game state plus the time until the next word, so the page can count down and roll over at midnight.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LetterlockResponse {
    #[serde(flatten)]
    state: letterlock::State,
    seconds_to_reset: i64,
}

/// The requested language, or the app language when none is given.
fn resolve_language(requested: Option<&str>, app: &AppSettings) -> Result<Language, AppError> {
    match requested {
        None => Ok(app.language),
        Some(code) => Language::from_code(code).ok_or_else(|| AppError::bad_request("invalid_language", format!("Unknown language {code:?}."))),
    }
}

async fn get_letterlock(State(state): State<AppState>, Query(query): Query<LanguageQuery>) -> Result<Json<LetterlockResponse>, AppError> {
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(query.lang.as_deref(), &settings::app(connection)?)?;
            let game = letterlock::state(connection, today, language, &settings::letterlock(connection)?)?;
            Ok(Json(LetterlockResponse { state: game, seconds_to_reset }))
        })
        .await
}

#[derive(Deserialize)]
struct GuessBody {
    word: String,
    lang: Option<String>,
}

async fn post_guess(State(state): State<AppState>, Json(body): Json<GuessBody>) -> Result<Json<LetterlockResponse>, AppError> {
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(body.lang.as_deref(), &settings::app(connection)?)?;
            let game_settings = settings::letterlock(connection)?;
            let game = letterlock::submit(connection, today, language, &body.word, &game_settings)?;
            Ok(Json(LetterlockResponse { state: game, seconds_to_reset }))
        })
        .await
}

/// Rarefind's state, the time until tomorrow's puzzle, and (after an answer) what happened to it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RarefindResponse {
    #[serde(flatten)]
    state: rarefind::State,
    seconds_to_reset: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<rarefind::AnswerResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claude_paused_until: Option<i64>,
}

#[derive(Deserialize)]
struct PositionBody {
    position: usize,
    lang: Option<String>,
}

#[derive(Deserialize)]
struct TextBody {
    position: usize,
    text: String,
    lang: Option<String>,
}

/// Runs one Rarefind action with the player's language and settings, and wraps the result for the browser.
async fn rarefind_call<F>(state: AppState, lang: Option<String>, action: F) -> Result<Json<RarefindResponse>, AppError>
where
    F: FnOnce(&mut rusqlite::Connection, chrono::NaiveDate, Language, &RarefindSettings, i64) -> Result<(rarefind::State, Option<rarefind::AnswerResult>), AppError>
        + Send
        + 'static,
{
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(lang.as_deref(), &settings::app(connection)?)?;
            let game_settings = settings::rarefind(connection)?;
            let now = now_ms();
            let (game, result) = action(connection, today, language, &game_settings, now)?;
            let claude_paused_until = claude_pause::active_until(connection, now)?;
            Ok(Json(RarefindResponse { state: game, seconds_to_reset, result, claude_paused_until }))
        })
        .await
}

async fn get_rarefind(State(state): State<AppState>, Query(query): Query<LanguageQuery>) -> Result<Json<RarefindResponse>, AppError> {
    rarefind_call(state, query.lang, |connection, today, language, settings, now| {
        Ok((rarefind::state(connection, today, language, settings, now)?, None))
    })
    .await
}

async fn post_rarefind_start(State(state): State<AppState>, Json(body): Json<PositionBody>) -> Result<Json<RarefindResponse>, AppError> {
    rarefind_call(state, body.lang, move |connection, today, language, settings, now| {
        Ok((rarefind::start(connection, today, language, body.position, settings, now)?, None))
    })
    .await
}

async fn post_rarefind_answer(State(state): State<AppState>, Json(body): Json<TextBody>) -> Result<Json<RarefindResponse>, AppError> {
    rarefind_call(state, body.lang, move |connection, today, language, settings, now| {
        let (game, result) = rarefind::answer(connection, today, language, body.position, &body.text, settings, now)?;
        Ok((game, Some(result)))
    })
    .await
}

async fn post_rarefind_expire(State(state): State<AppState>, Json(body): Json<PositionBody>) -> Result<Json<RarefindResponse>, AppError> {
    rarefind_call(state, body.lang, move |connection, today, language, settings, now| {
        Ok((rarefind::expire(connection, today, language, body.position, settings, now)?, None))
    })
    .await
}

async fn post_rarefind_contest(State(state): State<AppState>, Json(body): Json<TextBody>) -> Result<Json<RarefindResponse>, AppError> {
    rarefind_call(state, body.lang, move |connection, today, language, settings, now| {
        Ok((rarefind::contest(connection, today, language, body.position, &body.text, settings, now)?, None))
    })
    .await
}

#[derive(Deserialize)]
struct AutofillQuery {
    position: usize,
    q: String,
    lang: Option<String>,
}

#[derive(Serialize)]
struct AutofillResponse {
    completion: Option<String>,
}

async fn get_rarefind_autofill(State(state): State<AppState>, Query(query): Query<AutofillQuery>) -> Result<Json<AutofillResponse>, AppError> {
    let today = state.clock.today();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(query.lang.as_deref(), &settings::app(connection)?)?;
            let game_settings = settings::rarefind(connection)?;
            let completion = rarefind::autofill(connection, today, language, query.position, &query.q, &game_settings, now_ms())?;
            Ok(Json(AutofillResponse { completion }))
        })
        .await
}

async fn put_atlas_settings(State(state): State<AppState>, Json(value): Json<AtlasSettings>) -> Result<Json<AllSettings>, AppError> {
    state
        .db
        .call(move |connection| {
            settings::save_atlas(connection, &value)?;
            Ok(Json(settings::all(connection)?))
        })
        .await
}

/// Atlas Hunt's state, the time until tomorrow's clues, and (after a pick) what happened to it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AtlasResponse {
    #[serde(flatten)]
    state: atlas::State,
    seconds_to_reset: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<atlas::PickResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claude_paused_until: Option<i64>,
}

async fn get_atlas(State(state): State<AppState>, Query(query): Query<LanguageQuery>) -> Result<Json<AtlasResponse>, AppError> {
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(query.lang.as_deref(), &settings::app(connection)?)?;
            let game = atlas::state(connection, today, language, &settings::atlas(connection)?)?;
            let claude_paused_until = claude_pause::active_until(connection, now_ms())?;
            Ok(Json(AtlasResponse { state: game, seconds_to_reset, result: None, claude_paused_until }))
        })
        .await
}

#[derive(Deserialize)]
struct PickBody {
    position: usize,
    country: String,
    lang: Option<String>,
}

async fn post_atlas_pick(State(state): State<AppState>, Json(body): Json<PickBody>) -> Result<Json<AtlasResponse>, AppError> {
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let language = resolve_language(body.lang.as_deref(), &settings::app(connection)?)?;
            let game_settings = settings::atlas(connection)?;
            let (game, result) = atlas::pick(connection, today, language, body.position, body.country.trim(), &game_settings)?;
            let claude_paused_until = claude_pause::active_until(connection, now_ms())?;
            Ok(Json(AtlasResponse { state: game, seconds_to_reset, result: Some(result), claude_paused_until }))
        })
        .await
}

async fn put_snippet_settings(State(state): State<AppState>, Json(value): Json<SnippetSettings>) -> Result<Json<AllSettings>, AppError> {
    state
        .db
        .call(move |connection| {
            settings::save_snippet(connection, &value)?;
            Ok(Json(settings::all(connection)?))
        })
        .await
}

/// Snippet's state plus the time until tomorrow's song.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnippetResponse {
    #[serde(flatten)]
    state: snippet::State,
    seconds_to_reset: i64,
}

/// Runs one Snippet action with the player's settings and wraps the result for the browser.
async fn snippet_call<F>(state: AppState, action: F) -> Result<Json<SnippetResponse>, AppError>
where
    F: FnOnce(&mut rusqlite::Connection, chrono::NaiveDate, &SnippetSettings) -> Result<snippet::State, AppError> + Send + 'static,
{
    let today = state.clock.today();
    let seconds_to_reset = state.clock.seconds_until_next_day();
    state
        .db
        .call(move |connection| {
            let game_settings = settings::snippet(connection)?;
            let game = action(connection, today, &game_settings)?;
            Ok(Json(SnippetResponse { state: game, seconds_to_reset }))
        })
        .await
}

async fn get_snippet(State(state): State<AppState>) -> Result<Json<SnippetResponse>, AppError> {
    snippet_call(state, |connection, today, settings| snippet::state(connection, today, settings)).await
}

#[derive(Deserialize)]
struct GuessTextBody {
    text: String,
}

async fn post_snippet_guess(State(state): State<AppState>, Json(body): Json<GuessTextBody>) -> Result<Json<SnippetResponse>, AppError> {
    snippet_call(state, move |connection, today, settings| snippet::guess(connection, today, &body.text, settings)).await
}

async fn post_snippet_skip(State(state): State<AppState>) -> Result<Json<SnippetResponse>, AppError> {
    snippet_call(state, |connection, today, settings| snippet::skip(connection, today, settings)).await
}

fn bytes_response(content_type: &'static str, bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, NO_STORE)], bytes).into_response()
}

async fn get_snippet_clip(State(state): State<AppState>) -> Result<Response, AppError> {
    let today = state.clock.today();
    state
        .db
        .call(move |connection| {
            let clip = snippet::clip(connection, today, &settings::snippet(connection)?)?;
            Ok(bytes_response(MP3_TYPE, clip))
        })
        .await
}

async fn get_snippet_cover(State(state): State<AppState>) -> Result<Response, AppError> {
    let today = state.clock.today();
    state
        .db
        .call(move |connection| {
            Ok(match snippet::cover(connection, today, &settings::snippet(connection)?)? {
                Some(cover) => bytes_response(COVER_TYPE, cover),
                None => StatusCode::NOT_FOUND.into_response(),
            })
        })
        .await
}

#[derive(Deserialize)]
struct SuggestQuery {
    q: String,
}

async fn get_snippet_suggest(State(state): State<AppState>, Query(query): Query<SuggestQuery>) -> Result<Json<Vec<snippet::Suggestion>>, AppError> {
    state.db.call(move |connection| Ok(Json(snippet::suggest(connection, &query.q)?))).await
}
