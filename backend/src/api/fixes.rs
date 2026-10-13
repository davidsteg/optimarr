use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{bad_request, not_found, ApiError, ApiResult};
use crate::fixer::{self, EnqueueError, FixSettings};
use crate::state::AppState;

pub fn enqueue_error(e: EnqueueError) -> ApiError {
    match e {
        EnqueueError::NotFound => ApiError::NotFound(e.to_string()),
        EnqueueError::Invalid(m) => ApiError::BadRequest(m),
        EnqueueError::Other(e) => ApiError::Internal(e),
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    status: Option<String>,
    limit: Option<i64>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    let items = fixer::list(&state, q.status.as_deref().filter(|s| !s.is_empty()), q.limit.unwrap_or(100).clamp(1, 1000)).await?;
    let s: FixSettings = crate::settings::load(&state.pool).await?;
    Ok(Json(json!({
        "enabled": s.enabled,
        "ffmpeg": fixer::ffmpeg_available().await,
        "items": items,
    })))
}

#[derive(Deserialize)]
pub struct IdsBody {
    ids: Vec<i64>,
}

/// Queue fixes for several files; returns one result per id.
pub async fn enqueue(State(state): State<AppState>, Json(b): Json<IdsBody>) -> ApiResult<Json<Value>> {
    if b.ids.is_empty() {
        return Err(bad_request("No files selected"));
    }
    if b.ids.len() > 1000 {
        return Err(bad_request("At most 1000 files per request"));
    }
    let mut results = Vec::new();
    for id in b.ids {
        match fixer::enqueue(&state, id).await {
            Ok(f) => results.push(json!({ "id": id, "queued": true, "fixId": f.id })),
            Err(EnqueueError::Other(e)) => return Err(ApiError::Internal(e)),
            Err(e) => results.push(json!({ "id": id, "queued": false, "message": e.to_string() })),
        }
    }
    let queued = results.iter().filter(|r| r["queued"] == true).count();
    Ok(Json(json!({ "queued": queued, "results": results })))
}

pub async fn cancel(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    if !fixer::cancel(&state, id).await? {
        return Err(not_found("No queued or running fix with that id"));
    }
    Ok(Json(json!({ "cancelled": true })))
}

pub async fn clear_finished(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let r = sqlx::query("DELETE FROM fixes WHERE status IN ('done', 'failed', 'cancelled')").execute(&state.pool).await?;
    state.jobs.emit(crate::jobs::Event::FixesChanged);
    Ok(Json(json!({ "removed": r.rows_affected() })))
}

pub async fn get_settings(State(state): State<AppState>) -> ApiResult<Json<FixSettings>> {
    Ok(Json(crate::settings::load(&state.pool).await?))
}

pub async fn put_settings(State(state): State<AppState>, Json(s): Json<FixSettings>) -> ApiResult<Json<Value>> {
    crate::settings::save(&state.pool, &s).await?;
    state.fixes.wake();
    Ok(Json(json!({ "ok": true })))
}
