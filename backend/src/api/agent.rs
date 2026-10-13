//! Compact, stable endpoints designed for AI agents and scripts (documented in docs/API.md).
//! Every response carries a plain-language `summary` an agent can relay as-is.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::media::build_detail;
use super::{bad_request, not_found, ApiResult};
use crate::integrations::servarr;
use crate::media::model::{Issue, Severity};
use crate::media::rating::channel_label;
use crate::repo::{self, MediaFile, MediaQuery};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    score: i64,
    verdict: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBrief {
    id: i64,
    title: String,
    path: String,
    compatibility: Score,
    quality: Score,
    status: String,
    main_issues: Vec<String>,
}

fn brief(m: &MediaFile) -> FileBrief {
    FileBrief {
        id: m.id,
        title: m.title(),
        path: m.path.clone(),
        compatibility: Score { score: m.compat_score, verdict: m.compat_verdict.clone() },
        quality: Score { score: m.quality_score, verdict: m.quality_verdict.clone() },
        status: m.status.clone(),
        main_issues: m.issues.0.iter().filter(|i| i.severity >= Severity::Warning).map(|i| i.message.clone()).take(3).collect(),
    }
}

fn mbps(b: i64) -> String {
    format!("{:.1} Mbps", b as f64 / 1e6)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    id: i64,
    title: String,
    path: String,
    status: String,
    summary: String,
    compatibility: Score,
    quality: Score,
    technical: Value,
    issues: Vec<Issue>,
    ffmpeg_command: Option<String>,
    managed_by: Option<Value>,
    playback: Value,
}

async fn report(state: &AppState, m: MediaFile) -> anyhow::Result<Report> {
    let d = build_detail(state, m).await?;
    let f = &d.file;
    let video = if f.video_codec.is_empty() {
        Value::Null
    } else {
        json!(format!(
            "{} {}-bit {} ({}x{}){}{}",
            f.video_codec,
            f.bit_depth,
            d.resolution,
            f.width,
            f.height,
            if f.hdr_type.is_empty() { String::new() } else { format!(" {}", f.hdr_type) },
            if f.video_bitrate > 0 { format!(", {}", mbps(f.video_bitrate)) } else { String::new() }
        ))
    };
    let audio: Vec<String> = f
        .audio_tracks
        .0
        .iter()
        .map(|a| {
            format!("{} {}{}", a.codec, channel_label(a.channels), if a.language.is_empty() { String::new() } else { format!(" [{}]", a.language) })
        })
        .collect();
    let subs: Vec<String> = f
        .subtitle_tracks
        .0
        .iter()
        .map(|s| {
            format!(
                "{}{}{}",
                s.format,
                if s.language.is_empty() { String::new() } else { format!(" [{}]", s.language) },
                if s.embedded { "" } else { " (external)" }
            )
        })
        .collect();
    let managed_by = match f.servarr_kind.as_deref() {
        Some("sonarr") => Some(json!({ "service": "Sonarr", "seriesId": f.sonarr_series_id, "episodeId": f.sonarr_episode_id })),
        Some("radarr") => Some(json!({ "service": "Radarr", "movieId": f.radarr_movie_id })),
        _ => None,
    };
    Ok(Report {
        id: f.id,
        title: d.title.clone(),
        path: f.path.clone(),
        status: f.status.clone(),
        summary: d.assessment.summary.clone(),
        compatibility: Score { score: d.assessment.compat_score as i64, verdict: d.assessment.compat_verdict.clone() },
        quality: Score { score: d.assessment.quality_score as i64, verdict: d.assessment.quality_verdict.clone() },
        technical: json!({
            "container": f.container,
            "video": video,
            "audio": audio,
            "subtitles": subs,
            "durationMinutes": (f.duration / 60.0).round(),
            "sizeGB": (f.size as f64 / 1e9 * 100.0).round() / 100.0,
            "overallBitrate": (f.overall_bitrate > 0).then(|| mbps(f.overall_bitrate)),
        }),
        issues: d.assessment.issues.clone(),
        ffmpeg_command: d.assessment.ffmpeg_command.clone(),
        managed_by,
        playback: d.playback.clone(),
    })
}

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let (files, size, optimal, good, poor, q_exc, q_good, q_fair, q_poor, broken): (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(size),0),
                COALESCE(SUM(compat_verdict='optimal'),0), COALESCE(SUM(compat_verdict='good'),0), COALESCE(SUM(compat_verdict='poor'),0),
                COALESCE(SUM(quality_verdict='excellent'),0), COALESCE(SUM(quality_verdict='good'),0),
                COALESCE(SUM(quality_verdict='fair'),0), COALESCE(SUM(quality_verdict='poor'),0),
                COALESCE(SUM(status='broken'),0)
             FROM media_files WHERE status != 'missing'",
        )
        .fetch_one(&state.pool)
        .await?;
    let top_issues: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT json_extract(value,'$.code'), MIN(json_extract(value,'$.message')), COUNT(*) FROM media_files, json_each(media_files.issues)
         WHERE status != 'missing' AND json_extract(value,'$.severity') != 'info' GROUP BY 1 ORDER BY 3 DESC LIMIT 8",
    )
    .fetch_all(&state.pool)
    .await?;
    let worst: Vec<MediaFile> = sqlx::query_as("SELECT * FROM media_files WHERE status = 'ok' ORDER BY (compat_score + quality_score) ASC LIMIT 5")
        .fetch_all(&state.pool)
        .await?;
    let pct = |n: i64| if files > 0 { (n as f64 * 1000.0 / files as f64).round() / 10.0 } else { 0.0 };
    let summary = format!(
        "{files} files ({}). Compatibility: {}% optimal, {}% good, {}% poor. Quality: {}% excellent, {}% good, {}% fair, {}% poor. {broken} broken.",
        human_size(size),
        pct(optimal),
        pct(good),
        pct(poor),
        pct(q_exc),
        pct(q_good),
        pct(q_fair),
        pct(q_poor)
    );
    Ok(Json(json!({
        "summary": summary,
        "files": files,
        "sizeBytes": size,
        "compatibility": { "optimal": optimal, "good": good, "poor": poor },
        "quality": { "excellent": q_exc, "good": q_good, "fair": q_fair, "poor": q_poor },
        "broken": broken,
        "topIssues": top_issues.into_iter().map(|(code, example, count)| json!({ "code": code, "count": count, "example": example })).collect::<Vec<_>>(),
        "worstFiles": worst.iter().map(brief).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FilesQuery {
    q: Option<String>,
    compatibility: Option<String>,
    quality: Option<String>,
    issue: Option<String>,
    status: Option<String>,
    library_id: Option<i64>,
    /// worst (default) | best | largest | newest | name
    sort: Option<String>,
    limit: Option<i64>,
}

pub async fn files(State(state): State<AppState>, Query(f): Query<FilesQuery>) -> ApiResult<Json<Value>> {
    let (sort, dir) = match f.sort.as_deref().unwrap_or("worst") {
        "best" => ("compat", "desc"),
        "largest" => ("size", "desc"),
        "newest" => ("analyzed", "desc"),
        "name" => ("name", "asc"),
        _ => ("compat", "asc"),
    };
    let q = MediaQuery {
        search: f.q,
        compat: f.compatibility,
        quality: f.quality,
        issue: f.issue,
        status: f.status,
        library_id: f.library_id,
        sort: Some(sort.into()),
        dir: Some(dir.into()),
        page: Some(1),
        page_size: Some(f.limit.unwrap_or(20).clamp(1, 100)),
        ..Default::default()
    };
    let page = repo::query_media(&state.pool, &q).await?;
    Ok(Json(json!({
        "total": page.total,
        "returned": page.items.len(),
        "files": page.items.iter().map(brief).collect::<Vec<_>>(),
    })))
}

pub async fn file(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Report>> {
    let m = repo::get_media(&state.pool, id).await?.ok_or_else(|| not_found("No file with that id"))?;
    Ok(Json(report(&state, m).await?))
}

#[derive(Deserialize)]
pub struct LookupQuery {
    path: Option<String>,
    title: Option<String>,
}

/// Find a file by exact path, or by title (best match first, up to 5 reports).
pub async fn lookup(State(state): State<AppState>, Query(q): Query<LookupQuery>) -> ApiResult<Json<Value>> {
    if let Some(p) = q.path.filter(|p| !p.is_empty()) {
        let m = repo::get_media_by_path(&state.pool, &p).await?.ok_or_else(|| not_found("No analyzed file at that path"))?;
        return Ok(Json(json!({ "matches": [report(&state, m).await?] })));
    }
    let Some(title) = q.title.filter(|t| !t.trim().is_empty()) else {
        return Err(bad_request("Pass ?path= or ?title="));
    };
    let like = format!("%{}%", title.trim());
    let rows: Vec<MediaFile> = sqlx::query_as(
        "SELECT * FROM media_files WHERE radarr_movie_title LIKE ?1 OR sonarr_series_title LIKE ?1 OR file_name LIKE ?1
         ORDER BY (radarr_movie_title = ?2 OR sonarr_series_title = ?2) DESC, season_number, episode_number LIMIT 5",
    )
    .bind(&like)
    .bind(title.trim())
    .fetch_all(&state.pool)
    .await?;
    let mut matches = Vec::new();
    for m in rows {
        matches.push(report(&state, m).await?);
    }
    Ok(Json(json!({ "matches": matches })))
}

#[derive(Deserialize)]
pub struct DaysQuery {
    days: Option<i64>,
}

pub async fn transcodes(State(state): State<AppState>, Query(q): Query<DaysQuery>) -> ApiResult<Json<Value>> {
    let days = q.days.unwrap_or(30).clamp(1, 3650);
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let rows: Vec<(i64, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT media_file_id, COUNT(*), SUM(play_method = 'Transcode'), GROUP_CONCAT(DISTINCT transcode_reasons)
         FROM playback_events WHERE started_at >= ? AND media_file_id IS NOT NULL
         GROUP BY media_file_id HAVING SUM(play_method = 'Transcode') > 0 ORDER BY 3 DESC LIMIT 20",
    )
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    let mut out = Vec::new();
    for (id, plays, transcodes, reasons) in rows {
        if let Some(m) = repo::get_media(&state.pool, id).await? {
            out.push(json!({ "file": brief(&m), "plays": plays, "transcodes": transcodes, "jellyfinReasons": reasons }));
        }
    }
    Ok(Json(json!({ "days": days, "files": out })))
}

/// Re-analyze one file now and return the fresh report.
pub async fn rescan(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Report>> {
    let m = repo::get_media(&state.pool, id).await?.ok_or_else(|| not_found("No file with that id"))?;
    let detail = super::media::analyze_path(State(state.clone()), Json(super::media::AnalyzeBody { path: m.path })).await?;
    Ok(Json(report(&state, detail.0.file).await?))
}

pub async fn redownload(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let r = servarr::redownload(&state, &[id]).await?;
    Ok(Json(json!(r.into_iter().next())))
}

fn human_size(bytes: i64) -> String {
    let b = bytes as f64;
    if b >= 1e12 {
        format!("{:.1} TB", b / 1e12)
    } else if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else {
        format!("{:.0} MB", b / 1e6)
    }
}
