use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{bad_request, not_found, ApiResult};
use crate::devices::{self, Device, DeviceCapabilities, Outcome};
use crate::media::model::MediaInfo;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Device>>> {
    Ok(Json(devices::list(&state.pool).await?))
}

pub async fn templates() -> Json<Value> {
    Json(json!(devices::templates().into_iter().map(|(name, profile)| json!({ "name": name, "profile": profile })).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBody {
    name: String,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    profile: DeviceCapabilities,
}

fn yes() -> bool {
    true
}

pub async fn create(State(state): State<AppState>, Json(b): Json<DeviceBody>) -> ApiResult<Json<Value>> {
    if b.name.trim().is_empty() {
        return Err(bad_request("A name is required"));
    }
    let id = devices::create(&state.pool, b.name.trim(), b.enabled, &b.profile).await?;
    Ok(Json(json!({ "id": id })))
}

pub async fn update(State(state): State<AppState>, Path(id): Path<i64>, Json(b): Json<DeviceBody>) -> ApiResult<Json<Value>> {
    if b.name.trim().is_empty() {
        return Err(bad_request("A name is required"));
    }
    if !devices::update(&state.pool, id, b.name.trim(), b.enabled, &b.profile).await? {
        return Err(not_found("Device not found"));
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    if !devices::delete(&state.pool, id).await? {
        return Err(not_found("Device not found"));
    }
    Ok(Json(json!({ "ok": true })))
}

/// Per device: how many files direct-play, direct-stream (remux) or transcode, and why.
pub async fn summary(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(device_summary(&state).await?)))
}

pub async fn device_summary(state: &AppState) -> anyhow::Result<Vec<Value>> {
    let devs = devices::list_enabled(&state.pool).await?;
    if devs.is_empty() {
        return Ok(vec![]);
    }
    let mut counts = vec![[0i64; 3]; devs.len()];
    let mut sizes = vec![0i64; devs.len()];
    let mut reasons: Vec<HashMap<String, i64>> = vec![HashMap::new(); devs.len()];
    let mut last_id = 0i64;
    loop {
        let rows: Vec<crate::repo::MediaFile> = sqlx::query_as("SELECT * FROM media_files WHERE status = 'ok' AND id > ? ORDER BY id LIMIT 1000")
            .bind(last_id)
            .fetch_all(&state.pool)
            .await?;
        let Some(last) = rows.last() else { break };
        last_id = last.id;
        for m in &rows {
            let info = m.to_media_info();
            for (i, d) in devs.iter().enumerate() {
                let v = devices::evaluate(&info, &d.profile.0);
                counts[i][v.outcome as usize] += 1;
                if v.outcome == Outcome::Transcode {
                    sizes[i] += m.size;
                    for r in v.reasons {
                        *reasons[i].entry(r).or_default() += 1;
                    }
                }
            }
        }
    }
    Ok(devs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let mut top: Vec<(String, i64)> = reasons[i].drain().collect();
            top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            top.truncate(8);
            json!({
                "id": d.id,
                "name": d.name,
                "directPlay": counts[i][0],
                "directStream": counts[i][1],
                "transcode": counts[i][2],
                "transcodeSizeBytes": sizes[i],
                "topReasons": top.into_iter().map(|(reason, count)| json!({ "reason": reason, "count": count })).collect::<Vec<_>>(),
            })
        })
        .collect())
}

/// Verdict of every enabled device for one file.
pub async fn for_file(state: &AppState, info: &MediaInfo) -> anyhow::Result<Vec<Value>> {
    Ok(devices::list_enabled(&state.pool)
        .await?
        .into_iter()
        .map(|d| {
            let v = devices::evaluate(info, &d.profile.0);
            json!({ "id": d.id, "name": d.name, "outcome": v.outcome, "reasons": v.reasons, "subtitleNote": v.subtitle_note })
        })
        .collect())
}

/// Files that would transcode on one device, worst offenders (most reasons) first.
pub async fn transcoding_files(
    state: &AppState,
    device_id: i64,
    limit: usize,
) -> anyhow::Result<Option<(i64, Vec<(crate::repo::MediaFile, Vec<String>)>)>> {
    let Some(d) = devices::get(&state.pool, device_id).await? else { return Ok(None) };
    let mut total = 0i64;
    let mut out = Vec::new();
    let mut last_id = 0i64;
    loop {
        let rows: Vec<crate::repo::MediaFile> = sqlx::query_as("SELECT * FROM media_files WHERE status = 'ok' AND id > ? ORDER BY id LIMIT 1000")
            .bind(last_id)
            .fetch_all(&state.pool)
            .await?;
        let Some(last) = rows.last() else { break };
        last_id = last.id;
        for m in rows {
            let v = devices::evaluate(&m.to_media_info(), &d.profile.0);
            if v.outcome == Outcome::Transcode {
                total += 1;
                out.push((m, v.reasons));
            }
        }
    }
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(b.0.size.cmp(&a.0.size)));
    out.truncate(limit);
    Ok(Some((total, out)))
}
