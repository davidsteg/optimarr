//! Inbound webhooks: Tdarr (after transcoding) and Sonarr/Radarr "Connect → Webhook" (after import).

use std::path::PathBuf;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use super::{bad_request, ApiResult};
use crate::integrations::servarr::{self, Kind};
use crate::media::files::is_video;
use crate::scanner;
use crate::settings::map_path;
use crate::state::AppState;

/// Collect every string in the payload that looks like a video file path.
fn video_paths(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) if (s.starts_with('/') || s.contains(":\\")) && is_video(std::path::Path::new(s)) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| video_paths(x, out)),
        Value::Object(o) => o.values().for_each(|x| video_paths(x, out)),
        _ => {}
    }
}

/// Tdarr: any JSON body containing the file path(s), e.g. `{"file": "/media/movie.mkv"}`.
pub async fn tdarr(State(state): State<AppState>, Json(body): Json<Value>) -> ApiResult<Json<Value>> {
    let mut paths = Vec::new();
    video_paths(&body, &mut paths);
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(bad_request("No video file path found in the payload"));
    }
    let job = scanner::start_path_analysis(&state, paths.iter().map(PathBuf::from).collect(), "Analyze file(s) from Tdarr").await?;
    Ok(Json(json!({ "accepted": paths, "jobId": job })))
}

/// Sonarr/Radarr webhook: analyze imported/upgraded files right away.
pub async fn servarr(State(state): State<AppState>, Json(body): Json<Value>) -> ApiResult<Json<Value>> {
    let event = body["eventType"].as_str().unwrap_or("");
    if event == "Test" {
        return Ok(Json(json!({ "ok": true })));
    }
    let kind = if body.get("series").is_some() { Kind::Sonarr } else { Kind::Radarr };
    let settings = servarr::load_settings(&state, kind).await?;
    let mut remote = Vec::new();
    for key in ["episodeFile", "movieFile"] {
        if let Some(p) = body[key]["path"].as_str() {
            remote.push(p.to_string());
        }
    }
    if let Some(files) = body["episodeFiles"].as_array() {
        remote.extend(files.iter().filter_map(|f| f["path"].as_str().map(String::from)));
    }
    if remote.is_empty() {
        // Rename/other events: fall back to the relative path joined with the folder.
        let folder = body["series"]["path"].as_str().or_else(|| body["movie"]["folderPath"].as_str());
        let rel = body["episodeFile"]["relativePath"].as_str().or_else(|| body["movieFile"]["relativePath"].as_str());
        if let (Some(f), Some(r)) = (folder, rel) {
            remote.push(format!("{}/{}", f.trim_end_matches('/'), r));
        }
    }
    let local: Vec<PathBuf> = remote.iter().map(|p| PathBuf::from(map_path(&settings.path_mappings, p))).collect();
    if local.is_empty() {
        return Ok(Json(json!({ "ignored": event })));
    }
    let job = scanner::start_path_analysis(&state, local, &format!("Analyze {} import", kind.label())).await?;
    if job.is_some() && servarr::Client::from_state(&state, kind).await.is_ok() {
        // Refresh links so redownloads target the new file.
        let st = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let _ = servarr::start_match(&st).await;
        });
    }
    Ok(Json(json!({ "event": event, "jobId": job })))
}
