use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use super::ApiResult;
use crate::state::AppState;

#[derive(serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Bucket {
    key: String,
    count: i64,
    size: i64,
}

async fn buckets(state: &AppState, expr: &str, filter: &str) -> sqlx::Result<Vec<Bucket>> {
    sqlx::query_as(&format!(
        "SELECT {expr} AS key, COUNT(*) AS count, COALESCE(SUM(size), 0) AS size FROM media_files
         WHERE status != 'missing' {filter} GROUP BY 1 ORDER BY 2 DESC"
    ))
    .fetch_all(&state.pool)
    .await
}

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let (files, size, duration, avg_compat, avg_quality, broken, processing, missing, fixable): (i64, i64, f64, Option<f64>, Option<f64>, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(size),0), COALESCE(SUM(duration),0),
                AVG(CASE WHEN status='ok' THEN compat_score END), AVG(CASE WHEN status='ok' THEN quality_score END),
                COALESCE(SUM(status='broken'),0), COALESCE(SUM(status='processing'),0),
                (SELECT COUNT(*) FROM media_files WHERE status='missing'),
                COALESCE(SUM(EXISTS (SELECT 1 FROM json_each(media_files.issues) WHERE json_extract(value,'$.code') IN ('container','codec_tag','fast_start'))
                    AND NOT EXISTS (SELECT 1 FROM json_each(media_files.issues) WHERE json_extract(value,'$.code') IN ('video_codec','bit_depth'))), 0)
             FROM media_files WHERE status != 'missing'",
        )
        .fetch_one(&state.pool)
        .await?;

    let compat = buckets(&state, "compat_verdict", "").await?;
    let quality = buckets(&state, "quality_verdict", "").await?;
    let codecs = buckets(&state, "video_codec", "AND video_codec != ''").await?;
    let containers = buckets(&state, "container", "AND container != ''").await?;
    let resolution = buckets(
        &state,
        "CASE WHEN width >= 3200 OR height >= 2000 THEN '4K' WHEN width >= 1800 OR height >= 1000 THEN '1080p'
              WHEN width >= 1200 OR height >= 700 THEN '720p' WHEN width > 0 THEN 'SD' ELSE 'Unknown' END",
        "",
    )
    .await?;
    let hdr = buckets(&state, "CASE WHEN hdr_type = '' THEN 'SDR' ELSE hdr_type END", "AND video_codec != ''").await?;
    let audio = buckets(&state, "CASE WHEN instr(audio_codecs, ',') > 0 THEN substr(audio_codecs, 1, instr(audio_codecs, ',') - 1) ELSE audio_codecs END", "AND audio_codecs != ''").await?;

    #[derive(serde::Serialize, sqlx::FromRow)]
    #[serde(rename_all = "camelCase")]
    struct IssueCount {
        code: String,
        area: String,
        severity: String,
        count: i64,
    }
    let issues: Vec<IssueCount> = sqlx::query_as(
        "SELECT json_extract(value,'$.code') AS code, json_extract(value,'$.area') AS area,
                MAX(json_extract(value,'$.severity')) AS severity, COUNT(*) AS count
         FROM media_files, json_each(media_files.issues) WHERE status != 'missing' GROUP BY 1, 2 ORDER BY 4 DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let worst: Vec<crate::repo::MediaFile> = sqlx::query_as(
        "SELECT * FROM media_files WHERE status = 'ok' ORDER BY (compat_score + quality_score) ASC, size DESC LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await?;

    let (plays, transcodes): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(play_method = 'Transcode'), 0) FROM playback_events WHERE started_at >= ?",
    )
    .bind((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339())
    .fetch_one(&state.pool)
    .await?;

    let jobs = state.jobs.history(8).await?;

    Ok(Json(json!({
        "totals": {
            "files": files, "size": size, "duration": duration,
            "avgCompat": avg_compat, "avgQuality": avg_quality,
            "broken": broken, "processing": processing, "missing": missing,
            "remuxFixable": fixable,
        },
        "compatibility": compat,
        "quality": quality,
        "videoCodecs": codecs,
        "containers": containers,
        "resolutions": resolution,
        "hdr": hdr,
        "audioCodecs": audio,
        "issues": issues,
        "worst": worst.iter().map(super::MediaSummary::from).collect::<Vec<_>>(),
        "playback30d": { "plays": plays, "transcodes": transcodes },
        "recentJobs": jobs,
    })))
}
