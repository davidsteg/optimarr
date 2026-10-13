use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use super::{bad_request, ApiResult};
use crate::integrations::servarr::{self, Kind};
use crate::scanner;
use crate::settings::{self, CompatibilityProfile, GeneralSettings, JellyfinSettings, QualityProfile, ServarrSettings};
use crate::state::AppState;

pub const MASK: &str = "••••••••";

fn mask(s: &str) -> String {
    if s.is_empty() {
        String::new()
    } else {
        MASK.to_string()
    }
}

/// Keep the stored secret when the client sends back the mask.
fn unmask(new: String, old: &str) -> String {
    if new == MASK {
        old.to_string()
    } else {
        new.trim().to_string()
    }
}

pub async fn get_compat(State(state): State<AppState>) -> ApiResult<Json<CompatibilityProfile>> {
    Ok(Json(settings::load(&state.pool).await?))
}

pub async fn put_compat(State(state): State<AppState>, Json(p): Json<CompatibilityProfile>) -> ApiResult<Json<Value>> {
    if p.thresholds.good > p.thresholds.optimal {
        return Err(bad_request("The 'good' threshold must not exceed the 'optimal' threshold"));
    }
    settings::save(&state.pool, &p).await?;
    let job = scanner::rescore_all(&state).await?;
    Ok(Json(json!({ "ok": true, "jobId": job })))
}

pub async fn reset_compat(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    put_compat(State(state), Json(CompatibilityProfile::default())).await
}

pub async fn get_quality(State(state): State<AppState>) -> ApiResult<Json<QualityProfile>> {
    Ok(Json(settings::load(&state.pool).await?))
}

pub async fn put_quality(State(state): State<AppState>, Json(p): Json<QualityProfile>) -> ApiResult<Json<Value>> {
    let t = &p.thresholds;
    if !(t.excellent >= t.good && t.good >= t.fair) {
        return Err(bad_request("Thresholds must satisfy excellent ≥ good ≥ fair"));
    }
    if p.min_bits_per_pixel <= 0.0 {
        return Err(bad_request("Minimum bits per pixel must be positive"));
    }
    settings::save(&state.pool, &p).await?;
    let job = scanner::rescore_all(&state).await?;
    Ok(Json(json!({ "ok": true, "jobId": job })))
}

pub async fn reset_quality(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    put_quality(State(state), Json(QualityProfile::default())).await
}

pub async fn get_general(State(state): State<AppState>) -> ApiResult<Json<GeneralSettings>> {
    Ok(Json(settings::load(&state.pool).await?))
}

pub async fn put_general(State(state): State<AppState>, Json(g): Json<GeneralSettings>) -> ApiResult<Json<Value>> {
    if !matches!(g.probe_backend.as_str(), "auto" | "mediainfo" | "ffprobe") {
        return Err(bad_request("probeBackend must be auto, mediainfo or ffprobe"));
    }
    if g.scan_concurrency > 64 {
        return Err(bad_request("scanConcurrency must be 64 or less"));
    }
    settings::save(&state.pool, &g).await?;
    state.jobs.emit(crate::jobs::Event::LibrariesChanged);
    Ok(Json(json!({ "ok": true, "restartRequired": true })))
}

fn parse_kind(k: &str) -> ApiResult<Kind> {
    Kind::parse(k).ok_or_else(|| bad_request("Unknown service, expected sonarr or radarr"))
}

pub async fn get_servarr(State(state): State<AppState>, Path(kind): Path<String>) -> ApiResult<Json<ServarrSettings>> {
    let mut s = servarr::load_settings(&state, parse_kind(&kind)?).await?;
    s.api_key = mask(&s.api_key);
    s.basic_auth_password = s.basic_auth_password.as_deref().map(mask);
    Ok(Json(s))
}

pub async fn put_servarr(State(state): State<AppState>, Path(kind): Path<String>, Json(mut s): Json<ServarrSettings>) -> ApiResult<Json<Value>> {
    let kind = parse_kind(&kind)?;
    let old = servarr::load_settings(&state, kind).await?;
    s.api_key = unmask(s.api_key, &old.api_key);
    s.basic_auth_password = s.basic_auth_password.map(|p| unmask(p, old.basic_auth_password.as_deref().unwrap_or("")));
    s.base_url = s.base_url.trim().trim_end_matches('/').to_string();
    if s.enabled && (s.base_url.is_empty() || s.api_key.is_empty()) {
        return Err(bad_request("URL and API key are required to enable the integration"));
    }
    s.path_mappings.retain(|m| !m.remote.trim().is_empty() && !m.local.trim().is_empty());
    servarr::save_settings(&state, kind, s).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn get_jellyfin(State(state): State<AppState>) -> ApiResult<Json<JellyfinSettings>> {
    let mut s: JellyfinSettings = settings::load(&state.pool).await?;
    s.api_key = mask(&s.api_key);
    Ok(Json(s))
}

pub async fn put_jellyfin(State(state): State<AppState>, Json(mut s): Json<JellyfinSettings>) -> ApiResult<Json<Value>> {
    let old: JellyfinSettings = settings::load(&state.pool).await?;
    s.api_key = unmask(s.api_key, &old.api_key);
    s.base_url = s.base_url.trim().trim_end_matches('/').to_string();
    if s.enabled && (s.base_url.is_empty() || s.api_key.is_empty()) {
        return Err(bad_request("URL and API key are required to enable Jellyfin"));
    }
    s.path_mappings.retain(|m| !m.remote.trim().is_empty() && !m.local.trim().is_empty());
    settings::save(&state.pool, &s).await?;
    Ok(Json(json!({ "ok": true })))
}

/// For the "test" buttons: settings from the form, falling back to stored secrets.
pub async fn merged_servarr(state: &AppState, kind: Kind, body: Option<ServarrSettings>) -> anyhow::Result<ServarrSettings> {
    let old = servarr::load_settings(state, kind).await?;
    Ok(match body {
        Some(mut s) => {
            s.api_key = unmask(s.api_key, &old.api_key);
            s.basic_auth_password = s.basic_auth_password.map(|p| unmask(p, old.basic_auth_password.as_deref().unwrap_or("")));
            s
        }
        None => old,
    })
}

pub async fn merged_jellyfin(state: &AppState, body: Option<JellyfinSettings>) -> anyhow::Result<JellyfinSettings> {
    let old: JellyfinSettings = settings::load(&state.pool).await?;
    Ok(match body {
        Some(mut s) => {
            s.api_key = unmask(s.api_key, &old.api_key);
            s
        }
        None => old,
    })
}
