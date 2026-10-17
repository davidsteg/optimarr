use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{bad_request, not_found, ApiError, ApiResult};
use crate::db::now;
use crate::jobs::Event;
use crate::repo::{self, LibraryWithStats};
use crate::scanner;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<LibraryWithStats>>> {
    Ok(Json(repo::list_libraries(&state.pool).await?))
}

fn valid_category(c: &str) -> bool {
    matches!(c, "movies" | "series" | "other")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewLibrary {
    path: String,
    name: Option<String>,
    category: Option<String>,
    watch: Option<bool>,
    #[serde(default = "yes")]
    scan: bool,
}

fn yes() -> bool {
    true
}

pub async fn create(State(state): State<AppState>, Json(body): Json<NewLibrary>) -> ApiResult<Json<Value>> {
    let path = body.path.trim().trim_end_matches('/').to_string();
    let path = if path.is_empty() { "/".to_string() } else { path };
    let md = tokio::fs::metadata(&path).await.map_err(|e| bad_request(format!("Cannot access {path}: {e}")))?;
    if !md.is_dir() {
        return Err(bad_request(format!("{path} is not a directory")));
    }
    let category = body.category.unwrap_or_else(|| "other".into());
    if !valid_category(&category) {
        return Err(bad_request("category must be movies, series or other"));
    }
    let name = body
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path.clone()));
    let id: i64 = sqlx::query_scalar("INSERT INTO libraries (name, path, category, watch, created_at) VALUES (?, ?, ?, ?, ?) RETURNING id")
        .bind(&name)
        .bind(&path)
        .bind(&category)
        .bind(body.watch.unwrap_or(true))
        .bind(now())
        .fetch_one(&state.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(d) if d.is_unique_violation() => ApiError::Conflict(format!("{path} is already a library")),
            _ => e.into(),
        })?;
    repo::attach_orphan_files(&state.pool, id, &path).await?;
    state.jobs.emit(Event::LibrariesChanged);
    let job = if body.scan { Some(scanner::start_library_scan(&state, id, false).await?) } else { None };
    Ok(Json(json!({ "id": id, "jobId": job })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryUpdate {
    name: Option<String>,
    category: Option<String>,
    enabled: Option<bool>,
    watch: Option<bool>,
}

pub async fn update(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<LibraryUpdate>) -> ApiResult<Json<Value>> {
    let lib = repo::get_library(&state.pool, id).await?.ok_or_else(|| not_found("Library not found"))?;
    if let Some(c) = &body.category {
        if !valid_category(c) {
            return Err(bad_request("category must be movies, series or other"));
        }
    }
    sqlx::query("UPDATE libraries SET name = ?, category = ?, enabled = ?, watch = ? WHERE id = ?")
        .bind(body.name.filter(|n| !n.trim().is_empty()).unwrap_or(lib.name))
        .bind(body.category.unwrap_or(lib.category))
        .bind(body.enabled.unwrap_or(lib.enabled))
        .bind(body.watch.unwrap_or(lib.watch))
        .bind(id)
        .execute(&state.pool)
        .await?;
    state.jobs.emit(Event::LibrariesChanged);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQuery {
    #[serde(default)]
    keep_media: bool,
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<i64>, Query(q): Query<RemoveQuery>) -> ApiResult<Json<Value>> {
    if let Some(j) = state.jobs.find_running("scan", Some(id)) {
        j.cancel.cancel();
    }
    let mut tx = state.pool.begin().await?;
    if q.keep_media {
        sqlx::query("UPDATE media_files SET library_id = NULL WHERE library_id = ?").bind(id).execute(&mut *tx).await?;
    }
    let res = sqlx::query("DELETE FROM libraries WHERE id = ?").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    if res.rows_affected() == 0 {
        return Err(not_found("Library not found"));
    }
    state.jobs.emit(Event::LibrariesChanged);
    state.jobs.emit(Event::MediaChanged);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ScanQuery {
    #[serde(default)]
    full: bool,
}

pub async fn scan(State(state): State<AppState>, Path(id): Path<i64>, Query(q): Query<ScanQuery>) -> ApiResult<Json<Value>> {
    repo::get_library(&state.pool, id).await?.ok_or_else(|| not_found("Library not found"))?;
    let job = scanner::start_library_scan(&state, id, q.full).await?;
    Ok(Json(json!({ "jobId": job })))
}

pub async fn scan_all(State(state): State<AppState>, Query(q): Query<ScanQuery>) -> ApiResult<Json<Value>> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM libraries WHERE enabled = 1").fetch_all(&state.pool).await?;
    let mut jobs = Vec::new();
    for id in ids {
        jobs.push(scanner::start_library_scan(&state, id, q.full).await?);
    }
    Ok(Json(json!({ "jobIds": jobs })))
}
