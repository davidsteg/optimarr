use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::agent::brief;
use super::{bad_request, ApiResult};
use crate::settings;
use crate::state::AppState;
use crate::upgrades::{self, UpgradeSettings};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CandidatesQuery {
    max_score: Option<i64>,
    include_recent: bool,
    limit: Option<i64>,
}

pub async fn candidates(State(state): State<AppState>, Query(q): Query<CandidatesQuery>) -> ApiResult<Json<Value>> {
    let s: UpgradeSettings = settings::load(&state.pool).await?;
    let max = q.max_score.unwrap_or(s.max_quality_score).clamp(0, 100);
    let (total, files) = upgrades::candidates(&state, max, q.include_recent, q.limit.unwrap_or(50).clamp(1, 500)).await?;
    Ok(Json(json!({
        "maxScore": max,
        "cooldownDays": s.cooldown_days,
        "total": total,
        "files": files.iter().map(brief).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct IdsBody {
    ids: Vec<i64>,
}

pub async fn search(State(state): State<AppState>, Json(b): Json<IdsBody>) -> ApiResult<Json<Value>> {
    if b.ids.is_empty() {
        return Err(bad_request("No files selected"));
    }
    if b.ids.len() > 200 {
        return Err(bad_request("At most 200 files per request"));
    }
    let results = upgrades::search(&state, &b.ids).await?;
    let ok = results.iter().filter(|r| r.success).count();
    Ok(Json(json!({ "searched": ok, "failed": results.len() - ok, "results": results })))
}

pub async fn get_settings(State(state): State<AppState>) -> ApiResult<Json<UpgradeSettings>> {
    Ok(Json(settings::load(&state.pool).await?))
}

pub async fn put_settings(State(state): State<AppState>, Json(s): Json<UpgradeSettings>) -> ApiResult<Json<Value>> {
    settings::save(&state.pool, &s).await?;
    Ok(Json(json!({ "ok": true })))
}
