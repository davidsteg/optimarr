use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::settings::MASK;
use super::{bad_request, ApiResult};
use crate::notify::{self, NotificationSettings, Target};
use crate::settings;
use crate::state::AppState;

fn validate(t: &Target) -> Result<(), String> {
    if !matches!(t.kind.as_str(), "discord" | "ntfy" | "webhook") {
        return Err(format!("Unknown notification type '{}'", t.kind));
    }
    if !(t.url.starts_with("http://") || t.url.starts_with("https://")) {
        return Err(format!("{}: the URL must start with http:// or https://", if t.name.is_empty() { &t.kind } else { &t.name }));
    }
    Ok(())
}

pub async fn get_settings(State(state): State<AppState>) -> ApiResult<Json<NotificationSettings>> {
    let mut s: NotificationSettings = settings::load(&state.pool).await?;
    for t in &mut s.targets {
        if !t.token.is_empty() {
            t.token = MASK.into();
        }
        // Discord webhook URLs carry their secret in the path.
        if t.kind == "discord" && !t.url.is_empty() {
            t.url = mask_discord(&t.url);
        }
    }
    Ok(Json(s))
}

fn mask_discord(url: &str) -> String {
    match url.rsplit_once('/') {
        Some((base, _)) => format!("{base}/{MASK}"),
        None => MASK.into(),
    }
}

/// Restore masked secrets from the stored settings (matched by position and kind).
fn unmask(mut new: NotificationSettings, old: &NotificationSettings) -> NotificationSettings {
    for (i, t) in new.targets.iter_mut().enumerate() {
        let prev = old.targets.get(i).filter(|o| o.kind == t.kind);
        if t.token == MASK {
            t.token = prev.map(|p| p.token.clone()).unwrap_or_default();
        }
        if t.url.ends_with(MASK) {
            t.url = prev.map(|p| p.url.clone()).unwrap_or_default();
        }
        t.url = t.url.trim().to_string();
    }
    new
}

pub async fn put_settings(State(state): State<AppState>, Json(s): Json<NotificationSettings>) -> ApiResult<Json<Value>> {
    let old: NotificationSettings = settings::load(&state.pool).await?;
    let s = unmask(s, &old);
    for t in &s.targets {
        validate(t).map_err(bad_request)?;
    }
    settings::save(&state.pool, &s).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestBody {
    /// Settings from the form (unsaved); falls back to the stored ones.
    settings: Option<NotificationSettings>,
}

pub async fn test(State(state): State<AppState>, body: Option<Json<TestBody>>) -> ApiResult<Json<Value>> {
    let old: NotificationSettings = settings::load(&state.pool).await?;
    let s = match body.and_then(|b| b.0.settings) {
        Some(s) => unmask(s, &old),
        None => old,
    };
    let targets: Vec<Target> = s.targets.into_iter().filter(|t| t.enabled).collect();
    if targets.is_empty() {
        return Err(bad_request("Add and enable at least one notification target"));
    }
    for t in &targets {
        validate(t).map_err(bad_request)?;
    }
    let results = notify::send_to(&state, &targets, "test", "Optimarr test", "Notifications from Optimarr are working.", json!({})).await;
    Ok(Json(json!({ "results": results })))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    id: i64,
    event: String,
    title: String,
    message: String,
    sent_at: String,
    results: sqlx::types::Json<Value>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    limit: Option<i64>,
}

pub async fn history(State(state): State<AppState>, Query(q): Query<HistoryQuery>) -> ApiResult<Json<Vec<Sent>>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM notifications ORDER BY id DESC LIMIT ?")
            .bind(q.limit.unwrap_or(50).clamp(1, 500))
            .fetch_all(&state.pool)
            .await?,
    ))
}
