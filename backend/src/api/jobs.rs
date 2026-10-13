use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{not_found, ApiResult};
use crate::jobs::JobSnapshot;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListQuery {
    limit: Option<i64>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> ApiResult<Json<Vec<JobSnapshot>>> {
    Ok(Json(state.jobs.history(q.limit.unwrap_or(50).clamp(1, 500)).await?))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    id: i64,
    path: String,
    error: String,
    size: Option<i64>,
    failed_at: String,
}

pub async fn failures(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Vec<Failure>>> {
    Ok(Json(
        sqlx::query_as("SELECT id, path, error, size, failed_at FROM job_failures WHERE job_id = ? ORDER BY id LIMIT 5000")
            .bind(id)
            .fetch_all(&state.pool)
            .await?,
    ))
}

pub async fn cancel(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    if state.jobs.cancel(id) {
        Ok(Json(json!({ "cancelled": true })))
    } else {
        Err(not_found("No running job with that id"))
    }
}
