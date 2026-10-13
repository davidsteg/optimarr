use std::net::SocketAddr;
use std::path::PathBuf;

use axum::extract::{ConnectInfo, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{bad_request, ApiResult};
use crate::auth::{self, AuthSettings};
use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "version": state.config.version,
        "timestamp": chrono::Utc::now(),
    }))
}

/// Lets the UI discover whether it must ask for an API key.
pub async fn auth_status(State(state): State<AppState>, req: axum::extract::Request) -> Json<Value> {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().cloned();
    let s = auth::current(&state).await;
    Json(json!({
        "keyRequired": auth::key_required(&state, peer.map(|p| p.0)).await,
        "mode": s.mode,
    }))
}

pub async fn status(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let db_path = state.config.data_dir.join(crate::DB_FILE);
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let (media, libraries, playback): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM media_files), (SELECT COUNT(*) FROM libraries), (SELECT COUNT(*) FROM playback_events)",
    )
    .fetch_one(&state.pool)
    .await?;
    let legacy: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'legacy_import'")
        .fetch_optional(&state.pool)
        .await?;
    Ok(Json(json!({
        "version": state.config.version,
        "startedAt": state.started_at,
        "uptimeSeconds": (chrono::Utc::now() - state.started_at).num_seconds(),
        "probe": { "mediainfo": state.probers.mediainfo, "ffprobe": state.probers.ffprobe },
        "probeSlots": state.probe_slots.available_permits(),
        "dataDir": state.config.data_dir,
        "databaseSize": db_size,
        "counts": { "media": media, "libraries": libraries, "playbackEvents": playback },
        "runningJobs": state.jobs.running(),
        "legacyImport": legacy.and_then(|l| serde_json::from_str::<Value>(&l).ok()),
    })))
}

pub async fn security(State(state): State<AppState>) -> Json<Value> {
    let s = auth::current(&state).await;
    Json(json!({
        "apiKey": s.api_key,
        "mode": s.mode,
        "keyFromEnvironment": !state.config.api_key.is_empty(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityUpdate {
    mode: String,
}

pub async fn update_security(State(state): State<AppState>, Json(body): Json<SecurityUpdate>) -> ApiResult<Json<Value>> {
    if !matches!(body.mode.as_str(), "external" | "always" | "never") {
        return Err(bad_request("mode must be external, always or never"));
    }
    let mut s: AuthSettings = crate::settings::load(&state.pool).await?;
    s.mode = body.mode;
    crate::settings::save(&state.pool, &s).await?;
    Ok(security(State(state)).await)
}

pub async fn regenerate_key(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    if !state.config.api_key.is_empty() {
        return Err(bad_request("The API key is set through OPTIMARR_API_KEY and cannot be changed here"));
    }
    auth::store_key(&state.pool, &auth::generate_key()).await?;
    Ok(security(State(state)).await)
}

#[derive(Deserialize)]
pub struct BrowseQuery {
    path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    name: String,
    path: String,
}

/// Directory picker for adding libraries.
pub async fn browse(Query(q): Query<BrowseQuery>) -> ApiResult<Json<Value>> {
    let path = PathBuf::from(q.path.filter(|p| !p.is_empty()).unwrap_or_else(|| "/".into()));
    let p2 = path.clone();
    let dirs = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<DirEntry>> {
        let mut v: Vec<DirEntry> = std::fs::read_dir(&p2)?
            .filter_map(Result::ok)
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) || e.path().is_dir())
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| DirEntry { name: e.file_name().to_string_lossy().into_owned(), path: e.path().to_string_lossy().into_owned() })
            .collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(v)
    })
    .await?
    .map_err(|e| bad_request(format!("Cannot read {}: {e}", path.display())))?;
    Ok(Json(json!({
        "path": path,
        "parent": path.parent(),
        "directories": dirs,
    })))
}
