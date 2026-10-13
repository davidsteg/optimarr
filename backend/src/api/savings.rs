use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::agent::brief;
use super::{bad_request, ApiResult};
use crate::media::savings::{self, Target};
use crate::repo::MediaFile;
use crate::state::AppState;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SavingsQuery {
    /// hevc (default) | av1
    target: Option<String>,
    library_id: Option<i64>,
    limit: Option<usize>,
}

pub async fn overview(State(state): State<AppState>, Query(q): Query<SavingsQuery>) -> ApiResult<Json<Value>> {
    let target = match q.target.as_deref() {
        None | Some("") => Target::Hevc,
        Some(t) => Target::parse(t).ok_or_else(|| bad_request("target must be hevc or av1"))?,
    };
    Ok(Json(compute(&state, target, q.library_id, q.limit.unwrap_or(25).clamp(1, 500)).await?))
}

pub async fn compute(state: &AppState, target: Target, library_id: Option<i64>, limit: usize) -> anyhow::Result<Value> {
    let mut files = 0i64;
    let mut current = 0u64;
    let mut saved = 0u64;
    let mut top: Vec<(MediaFile, savings::Estimate)> = Vec::new();
    let mut by_codec: std::collections::BTreeMap<String, (i64, u64)> = Default::default();
    let mut last_id = 0i64;
    loop {
        let rows: Vec<MediaFile> =
            sqlx::query_as("SELECT * FROM media_files WHERE status = 'ok' AND id > ?1 AND (?2 IS NULL OR library_id = ?2) ORDER BY id LIMIT 1000")
                .bind(last_id)
                .bind(library_id)
                .fetch_all(&state.pool)
                .await?;
        let Some(last) = rows.last() else { break };
        last_id = last.id;
        for m in rows {
            if let Some(e) = savings::estimate(&m.to_media_info(), m.size as u64, target) {
                files += 1;
                current += e.current_size;
                saved += e.savings;
                let c = by_codec.entry(m.video_codec.clone()).or_default();
                c.0 += 1;
                c.1 += e.savings;
                top.push((m, e));
            }
        }
        // Keep memory bounded on big libraries.
        if top.len() > limit * 4 {
            top.sort_by_key(|a| std::cmp::Reverse(a.1.savings));
            top.truncate(limit);
        }
    }
    top.sort_by_key(|a| std::cmp::Reverse(a.1.savings));
    top.truncate(limit);
    let summary = if files == 0 {
        format!("Re-encoding to {} would not save meaningful space.", target.label())
    } else {
        format!(
            "Re-encoding {files} files to {} would save about {} ({}% of {}).",
            target.label(),
            crate::notify::human_size(saved as i64),
            (saved as f64 * 100.0 / current.max(1) as f64).round(),
            crate::notify::human_size(current as i64)
        )
    };
    Ok(json!({
        "target": target,
        "summary": summary,
        "files": files,
        "currentBytes": current,
        "savingsBytes": saved,
        "byCodec": by_codec.into_iter().map(|(codec, (n, s))| json!({ "codec": codec, "files": n, "savingsBytes": s })).collect::<Vec<_>>(),
        "top": top.iter().map(|(m, e)| json!({ "file": brief(m), "estimate": e })).collect::<Vec<_>>(),
    }))
}
