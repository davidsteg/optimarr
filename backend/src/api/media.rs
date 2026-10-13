use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{bad_request, not_found, ApiResult};
use crate::integrations::servarr;
use crate::media::analyzer::{self, Outcome, Profiles};
use crate::media::model::{Assessment, Issue};
use crate::media::rating::{self, ResolutionTier};
use crate::repo::{self, MediaFile, MediaQuery, Page};
use crate::scanner;
use crate::settings::{self, CompatibilityProfile, QualityProfile};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub file_name: String,
    pub path: String,
    pub size: i64,
    pub duration: f64,
    pub container: String,
    pub video_codec: String,
    pub resolution: String,
    pub width: i64,
    pub height: i64,
    pub bit_depth: i64,
    pub hdr_type: Option<String>,
    pub overall_bitrate: i64,
    pub audio_codecs: Vec<String>,
    pub subtitle_formats: Vec<String>,
    pub compat_score: i64,
    pub compat_verdict: String,
    pub quality_score: i64,
    pub quality_verdict: String,
    pub status: String,
    pub status_reason: Option<String>,
    pub issues: Vec<IssueBrief>,
    pub servarr_kind: Option<String>,
    pub analyzed_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueBrief {
    pub code: String,
    pub area: crate::media::model::IssueArea,
    pub severity: crate::media::model::Severity,
    pub message: String,
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect()
}

impl From<&MediaFile> for MediaSummary {
    fn from(m: &MediaFile) -> Self {
        let resolution = if m.width > 0 || m.height > 0 {
            ResolutionTier::of(m.width as u32, m.height as u32).label().to_string()
        } else {
            String::new()
        };
        Self {
            id: m.id,
            library_id: m.library_id,
            title: m.title(),
            file_name: m.file_name.clone(),
            path: m.path.clone(),
            size: m.size,
            duration: m.duration,
            container: m.container.clone(),
            video_codec: m.video_codec.clone(),
            resolution,
            width: m.width,
            height: m.height,
            bit_depth: m.bit_depth,
            hdr_type: (!m.hdr_type.is_empty()).then(|| m.hdr_type.clone()),
            overall_bitrate: m.overall_bitrate,
            audio_codecs: split_csv(&m.audio_codecs),
            subtitle_formats: split_csv(&m.subtitle_formats),
            compat_score: m.compat_score,
            compat_verdict: m.compat_verdict.clone(),
            quality_score: m.quality_score,
            quality_verdict: m.quality_verdict.clone(),
            status: m.status.clone(),
            status_reason: m.status_reason.clone(),
            issues: m
                .issues
                .0
                .iter()
                .map(|i| IssueBrief { code: i.code.clone(), area: i.area, severity: i.severity, message: i.message.clone() })
                .collect(),
            servarr_kind: m.servarr_kind.clone(),
            analyzed_at: m.analyzed_at.clone(),
        }
    }
}

pub async fn list(State(state): State<AppState>, Query(q): Query<MediaQuery>) -> ApiResult<Json<Page<MediaSummary>>> {
    let page = repo::query_media(&state.pool, &q).await?;
    Ok(Json(Page {
        items: page.items.iter().map(MediaSummary::from).collect(),
        total: page.total,
        page: page.page,
        page_size: page.page_size,
    }))
}

pub async fn ids(State(state): State<AppState>, Query(q): Query<MediaQuery>) -> ApiResult<Json<Value>> {
    let ids = repo::query_media_ids(&state.pool, &q).await?;
    Ok(Json(json!({ "ids": ids, "total": ids.len() })))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Facet {
    value: String,
    count: i64,
}

async fn facet(state: &AppState, sql: &str) -> sqlx::Result<Vec<Facet>> {
    sqlx::query_as(sql).fetch_all(&state.pool).await
}

/// Distinct values (with counts) for building filter menus.
pub async fn facets(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let codecs = facet(&state, "SELECT video_codec AS value, COUNT(*) AS count FROM media_files WHERE video_codec != '' GROUP BY 1 ORDER BY 2 DESC").await?;
    let containers = facet(&state, "SELECT container AS value, COUNT(*) AS count FROM media_files WHERE container != '' GROUP BY 1 ORDER BY 2 DESC").await?;
    let audio = facet(
        &state,
        "WITH RECURSIVE split(word, rest) AS (
            SELECT '', audio_codecs || ',' FROM media_files
            UNION ALL SELECT substr(rest, 0, instr(rest, ',')), substr(rest, instr(rest, ',') + 1) FROM split WHERE rest != '')
         SELECT word AS value, COUNT(*) AS count FROM split WHERE word != '' GROUP BY 1 ORDER BY 2 DESC",
    )
    .await?;
    let hdr = facet(&state, "SELECT hdr_type AS value, COUNT(*) AS count FROM media_files WHERE hdr_type != '' GROUP BY 1 ORDER BY 2 DESC").await?;
    let issues = facet(
        &state,
        "SELECT json_extract(value, '$.code') AS value, COUNT(*) AS count FROM media_files, json_each(media_files.issues) GROUP BY 1 ORDER BY 2 DESC",
    )
    .await?;
    Ok(Json(json!({
        "videoCodecs": codecs,
        "containers": containers,
        "audioCodecs": audio,
        "hdrTypes": hdr,
        "issues": issues,
    })))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetail {
    #[serde(flatten)]
    pub file: MediaFile,
    pub title: String,
    pub resolution: String,
    pub assessment: Assessment,
    pub playback: Value,
    pub servarr_remote_path: Option<String>,
}

pub async fn build_detail(state: &AppState, file: MediaFile) -> anyhow::Result<MediaDetail> {
    let compat: CompatibilityProfile = settings::load(&state.pool).await?;
    let quality: QualityProfile = settings::load(&state.pool).await?;
    let mut assessment = rating::assess(&file.to_media_info(), file.size as u64, &file.path, &compat, &quality);
    if file.status == "broken" {
        assessment.summary = format!(
            "Broken: {}. {}",
            file.status_reason.clone().unwrap_or_else(|| "the file could not be read".into()),
            assessment.summary
        );
    }
    let playback: Value = sqlx::query_as::<_, (i64, i64, Option<String>)>(
        "SELECT COUNT(*), COALESCE(SUM(play_method = 'Transcode'), 0), MAX(started_at) FROM playback_events WHERE media_file_id = ?",
    )
    .bind(file.id)
    .fetch_one(&state.pool)
    .await
    .map(|(plays, transcodes, last)| json!({ "plays": plays, "transcodes": transcodes, "lastPlayed": last }))?;
    let remote = match file.servarr_kind.as_deref().and_then(servarr::Kind::parse) {
        Some(k) => servarr::load_settings(state, k).await.ok().map(|s| servarr::remote_path(&s, &file.path)),
        None => None,
    };
    Ok(MediaDetail {
        title: file.title(),
        resolution: ResolutionTier::of(file.width as u32, file.height as u32).label().into(),
        assessment,
        playback,
        servarr_remote_path: remote,
        file,
    })
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<MediaDetail>> {
    let file = repo::get_media(&state.pool, id).await?.ok_or_else(|| not_found("File not found"))?;
    Ok(Json(build_detail(&state, file).await?))
}

#[derive(Deserialize)]
pub struct IdsBody {
    ids: Vec<i64>,
}

pub async fn rescan(State(state): State<AppState>, Json(body): Json<IdsBody>) -> ApiResult<Json<Value>> {
    if body.ids.is_empty() {
        return Err(bad_request("No files selected"));
    }
    let n = body.ids.len();
    let job = scanner::start_rescan(&state, body.ids, &format!("Re-analyze {n} file(s)")).await?;
    Ok(Json(json!({ "jobId": job })))
}

pub async fn redownload(State(state): State<AppState>, Json(body): Json<IdsBody>) -> ApiResult<Json<Value>> {
    if body.ids.is_empty() {
        return Err(bad_request("No files selected"));
    }
    if body.ids.len() > 500 {
        return Err(bad_request("At most 500 files per request"));
    }
    let results = servarr::redownload(&state, &body.ids).await?;
    let ok = results.iter().filter(|r| r.success).count();
    Ok(Json(json!({ "succeeded": ok, "failed": results.len() - ok, "results": results })))
}

pub async fn purge_missing(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let res = sqlx::query("DELETE FROM media_files WHERE status = 'missing'").execute(&state.pool).await?;
    state.jobs.emit(crate::jobs::Event::MediaChanged);
    Ok(Json(json!({ "removed": res.rows_affected() })))
}

#[derive(Deserialize)]
pub struct AnalyzeBody {
    pub path: String,
}

/// Analyze a single file synchronously. The file must be inside a library.
pub async fn analyze_path(State(state): State<AppState>, Json(body): Json<AnalyzeBody>) -> ApiResult<Json<MediaDetail>> {
    let lib = repo::library_for_path(&state.pool, &body.path)
        .await?
        .ok_or_else(|| bad_request("The file must be inside one of your libraries"))?;
    let path = PathBuf::from(&body.path);
    let item = scanner::WorkItem::from_path(path.clone(), Some(lib.id)).ok_or_else(|| not_found("File not found"))?;
    let siblings: Vec<String> = path
        .parent()
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let compat: CompatibilityProfile = settings::load(&state.pool).await?;
    let quality: QualityProfile = settings::load(&state.pool).await?;
    let backends = state.probe_backends().await;
    let _permit = state.probe_slots.acquire().await?;
    match analyzer::analyze(&path, item.size, &siblings, &backends, &Profiles { compat: &compat, quality: &quality }).await {
        Outcome::Analyzed { info, assessment, broken } => {
            repo::upsert_analysis(
                &state.pool,
                &repo::NewAnalysis {
                    library_id: Some(lib.id),
                    path: &body.path,
                    size: item.size as i64,
                    mtime: item.mtime,
                    info: &info,
                    assessment: &assessment,
                    broken: broken.as_deref(),
                },
            )
            .await?;
        }
        Outcome::Failed(e) => return Err(bad_request(e)),
    }
    state.jobs.emit(crate::jobs::Event::MediaChanged);
    let file = repo::get_media_by_path(&state.pool, &body.path).await?.ok_or_else(|| not_found("File not found"))?;
    Ok(Json(build_detail(&state, file).await?))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export_csv(State(state): State<AppState>, Query(q): Query<MediaQuery>) -> ApiResult<Response> {
    let rows = repo::query_media_all(&state.pool, &q).await?;
    let mut out = String::from(
        "id,title,path,size_bytes,duration_s,container,video_codec,resolution,bit_depth,hdr,audio_codecs,subtitles,compat_score,compat_verdict,quality_score,quality_verdict,status,issues\n",
    );
    for m in &rows {
        let issues: Vec<&str> = m.issues.0.iter().map(|i: &Issue| i.code.as_str()).collect();
        let fields = [
            m.id.to_string(),
            m.title(),
            m.path.clone(),
            m.size.to_string(),
            format!("{:.0}", m.duration),
            m.container.clone(),
            m.video_codec.clone(),
            format!("{}x{}", m.width, m.height),
            m.bit_depth.to_string(),
            m.hdr_type.clone(),
            m.audio_codecs.clone(),
            m.subtitle_formats.clone(),
            m.compat_score.to_string(),
            m.compat_verdict.clone(),
            m.quality_score.to_string(),
            m.quality_verdict.clone(),
            m.status.clone(),
            issues.join(" "),
        ];
        out.push_str(&fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"optimarr-media.csv\"")
        .body(Body::from(out))?)
}
