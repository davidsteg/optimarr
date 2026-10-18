use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::settings::{merged_jellystat, merged_servarr};
use super::{bad_request, ApiResult};
use crate::integrations::{jellystat, servarr};
use crate::settings::{JellystatSettings, ServarrSettings};
use crate::state::AppState;

fn kind(k: &str) -> ApiResult<servarr::Kind> {
    servarr::Kind::parse(k).ok_or_else(|| bad_request("Unknown service, expected sonarr or radarr"))
}

/// Test a connection; accepts unsaved settings from the form.
pub async fn test_servarr(State(state): State<AppState>, Path(k): Path<String>, body: Option<Json<ServarrSettings>>) -> ApiResult<Json<Value>> {
    let kind = kind(&k)?;
    let s = merged_servarr(&state, kind, body.map(|b| b.0)).await?;
    if s.base_url.is_empty() || s.api_key.is_empty() {
        return Err(bad_request("URL and API key are required"));
    }
    let client = servarr::Client::new(state.http.clone(), kind, s);
    match client.status().await {
        Ok(version) => {
            let folders = client.root_folders().await.unwrap_or_default();
            Ok(Json(json!({ "ok": true, "version": version, "rootFolders": folders })))
        }
        Err(e) => Ok(Json(json!({ "ok": false, "error": format!("{e:#}") }))),
    }
}

pub async fn sync_libraries(State(state): State<AppState>, Path(k): Path<String>) -> ApiResult<Json<Value>> {
    let r = servarr::sync_root_folders(&state, kind(&k)?).await.map_err(|e| bad_request(format!("{e:#}")))?;
    Ok(Json(json!({ "synced": r.added.len(), "added": r.added, "existing": r.existing, "skipped": r.skipped })))
}

pub async fn match_files(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "jobId": servarr::start_match(&state).await? })))
}

pub async fn test_jellystat(State(state): State<AppState>, body: Option<Json<JellystatSettings>>) -> ApiResult<Json<Value>> {
    let s = merged_jellystat(&state, body.map(|b| b.0)).await?;
    if s.base_url.trim().is_empty() || s.api_key.trim().is_empty() {
        return Err(bad_request("URL and API key are required"));
    }
    match jellystat::Client::new(state.http.clone(), s).test().await {
        Ok(_) => Ok(Json(json!({ "ok": true }))),
        Err(e) => Ok(Json(json!({ "ok": false, "error": format!("{e:#}") }))),
    }
}

#[derive(Deserialize)]
pub struct ImportQuery {
    days: Option<i64>,
}

pub async fn import_jellystat(State(state): State<AppState>, Query(q): Query<ImportQuery>) -> ApiResult<Json<Value>> {
    let s = jellystat::load_settings(&state.pool).await?;
    if !s.enabled {
        return Err(bad_request("Connect Jellystat under Settings → Jellystat to import playback history"));
    }
    Ok(Json(json!({ "jobId": jellystat::start_history_import(&state, q.days.unwrap_or(90)).await? })))
}
