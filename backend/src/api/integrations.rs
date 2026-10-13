use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::settings::{merged_jellyfin, merged_servarr};
use super::{bad_request, ApiResult};
use crate::integrations::{jellyfin, servarr};
use crate::settings::{JellyfinSettings, ServarrSettings};
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
    let n = servarr::sync_root_folders(&state, kind(&k)?).await.map_err(|e| bad_request(format!("{e:#}")))?;
    Ok(Json(json!({ "synced": n })))
}

pub async fn match_files(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "jobId": servarr::start_match(&state).await? })))
}

pub async fn test_jellyfin(State(state): State<AppState>, body: Option<Json<JellyfinSettings>>) -> ApiResult<Json<Value>> {
    let s = merged_jellyfin(&state, body.map(|b| b.0)).await?;
    if s.base_url.is_empty() || s.api_key.is_empty() {
        return Err(bad_request("URL and API key are required"));
    }
    match jellyfin::Client::new(state.http.clone(), s).info().await {
        Ok(info) => Ok(Json(json!({ "ok": true, "info": info }))),
        Err(e) => Ok(Json(json!({ "ok": false, "error": format!("{e:#}") }))),
    }
}

#[derive(Deserialize)]
pub struct ImportQuery {
    days: Option<i64>,
}

pub async fn import_jellyfin(State(state): State<AppState>, Query(q): Query<ImportQuery>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "jobId": jellyfin::start_history_import(&state, q.days.unwrap_or(90)).await? })))
}
