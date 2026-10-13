use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};

use crate::state::AppState;

mod agent;
mod dashboard;
mod events;
mod integrations;
mod jobs;
mod libraries;
mod media;
mod playback;
mod settings;
mod system;
mod webhooks;

pub use media::MediaSummary;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError::Internal(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(e) => {
                tracing::error!(error = %format!("{e:#}"), "request failed");
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
            }
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError::BadRequest(msg.into())
}

pub fn not_found(msg: impl Into<String>) -> ApiError {
    ApiError::NotFound(msg.into())
}

pub fn router() -> Router<AppState> {
    Router::new()
        // system
        .route("/api/system/health", get(system::health))
        .route("/api/system/auth", get(system::auth_status))
        .route("/api/system/status", get(system::status))
        .route("/api/system/security", get(system::security).put(system::update_security))
        .route("/api/system/security/regenerate", post(system::regenerate_key))
        .route("/api/fs/browse", get(system::browse))
        // events + jobs
        .route("/api/events", get(events::stream))
        .route("/api/jobs", get(jobs::list))
        .route("/api/jobs/{id}/failures", get(jobs::failures))
        .route("/api/jobs/{id}/cancel", post(jobs::cancel))
        // libraries
        .route("/api/libraries", get(libraries::list).post(libraries::create))
        .route("/api/libraries/scan", post(libraries::scan_all))
        .route("/api/libraries/{id}", put(libraries::update).delete(libraries::remove))
        .route("/api/libraries/{id}/scan", post(libraries::scan))
        // media
        .route("/api/media", get(media::list))
        .route("/api/media/ids", get(media::ids))
        .route("/api/media/facets", get(media::facets))
        .route("/api/media/export.csv", get(media::export_csv))
        .route("/api/media/analyze", post(media::analyze_path))
        .route("/api/media/rescan", post(media::rescan))
        .route("/api/media/redownload", post(media::redownload))
        .route("/api/media/missing", delete(media::purge_missing))
        .route("/api/media/{id}", get(media::detail))
        // dashboard
        .route("/api/dashboard", get(dashboard::overview))
        // settings
        .route("/api/settings/compatibility", get(settings::get_compat).put(settings::put_compat))
        .route("/api/settings/compatibility/reset", post(settings::reset_compat))
        .route("/api/settings/quality", get(settings::get_quality).put(settings::put_quality))
        .route("/api/settings/quality/reset", post(settings::reset_quality))
        .route("/api/settings/general", get(settings::get_general).put(settings::put_general))
        .route("/api/settings/servarr/{kind}", get(settings::get_servarr).put(settings::put_servarr))
        .route("/api/settings/jellyfin", get(settings::get_jellyfin).put(settings::put_jellyfin))
        // integrations
        .route("/api/integrations/servarr/{kind}/test", post(integrations::test_servarr))
        .route("/api/integrations/servarr/{kind}/sync-libraries", post(integrations::sync_libraries))
        .route("/api/integrations/servarr/match", post(integrations::match_files))
        .route("/api/integrations/jellyfin/test", post(integrations::test_jellyfin))
        .route("/api/integrations/jellyfin/import", post(integrations::import_jellyfin))
        // playback
        .route("/api/playback", get(playback::list))
        .route("/api/playback/stats", get(playback::stats))
        // agent API (stable, compact)
        .route("/api/agent/overview", get(agent::overview))
        .route("/api/agent/files", get(agent::files))
        .route("/api/agent/files/{id}", get(agent::file))
        .route("/api/agent/files/{id}/rescan", post(agent::rescan))
        .route("/api/agent/files/{id}/redownload", post(agent::redownload))
        .route("/api/agent/lookup", get(agent::lookup))
        .route("/api/agent/transcodes", get(agent::transcodes))
        // webhooks
        .route("/api/webhooks/tdarr", post(webhooks::tdarr))
        .route("/api/library/webhook/tdarr", post(webhooks::tdarr))
        .route("/api/webhooks/servarr", post(webhooks::servarr))
}
