//! Database access for libraries and media files.

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{QueryBuilder, Sqlite, SqliteExecutor, SqlitePool};

use crate::db::now;
use crate::media::model::*;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub category: String,
    pub enabled: bool,
    pub watch: bool,
    pub servarr_kind: Option<String>,
    pub servarr_root_folder_id: Option<i64>,
    pub servarr_root_folder_path: Option<String>,
    pub created_at: String,
    pub last_scanned_at: Option<String>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LibraryWithStats {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub library: Library,
    pub file_count: i64,
    pub total_size: i64,
    pub optimal: i64,
    pub good: i64,
    pub poor: i64,
    pub broken: i64,
    pub avg_compat: Option<f64>,
    pub avg_quality: Option<f64>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub id: i64,
    pub library_id: Option<i64>,
    pub path: String,
    pub file_name: String,
    pub size: i64,
    pub mtime: i64,
    pub duration: f64,
    pub container: String,
    pub video_codec: String,
    pub video_codec_tag: String,
    pub codec_tag_ok: bool,
    pub video_profile: String,
    pub bit_depth: i64,
    pub width: i64,
    pub height: i64,
    pub frame_rate: f64,
    pub video_bitrate: i64,
    pub overall_bitrate: i64,
    pub hdr: bool,
    pub hdr_type: String,
    pub fast_start: bool,
    pub audio_codecs: String,
    pub audio_tracks: Json<Vec<AudioTrack>>,
    pub subtitle_formats: String,
    pub subtitle_tracks: Json<Vec<SubtitleTrack>>,
    pub compat_score: i64,
    pub compat_verdict: String,
    pub quality_score: i64,
    pub quality_verdict: String,
    pub issues: Json<Vec<Issue>>,
    pub status: String,
    pub status_reason: Option<String>,
    pub processing_since: Option<String>,
    pub analyzed_at: String,
    pub servarr_kind: Option<String>,
    pub sonarr_series_id: Option<i64>,
    pub sonarr_series_title: Option<String>,
    pub sonarr_episode_id: Option<i64>,
    pub sonarr_episode_file_id: Option<i64>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub radarr_movie_id: Option<i64>,
    pub radarr_movie_file_id: Option<i64>,
    pub radarr_movie_title: Option<String>,
    pub year: Option<i64>,
    pub matched_at: Option<String>,
    pub first_seen_at: Option<String>,
    pub upgrade_requested_at: Option<String>,
}

impl MediaFile {
    /// Rebuild the probe result from stored columns (used to re-score without re-probing).
    pub fn to_media_info(&self) -> MediaInfo {
        let video = (!self.video_codec.is_empty()).then(|| VideoStream {
            codec: self.video_codec.clone(),
            codec_tag: self.video_codec_tag.clone(),
            profile: self.video_profile.clone(),
            width: self.width as u32,
            height: self.height as u32,
            frame_rate: self.frame_rate,
            bit_depth: self.bit_depth as u32,
            bitrate: self.video_bitrate as u64,
            hdr_type: (!self.hdr_type.is_empty()).then(|| self.hdr_type.clone()),
        });
        MediaInfo {
            container: self.container.clone(),
            duration: self.duration,
            overall_bitrate: self.overall_bitrate as u64,
            fast_start: crate::media::rating::is_mp4_family(&self.container).then_some(self.fast_start),
            video,
            audio: self.audio_tracks.0.clone(),
            subtitles: self.subtitle_tracks.0.clone(),
        }
    }

    pub fn title(&self) -> String {
        if let Some(t) = &self.sonarr_series_title {
            return match (self.season_number, self.episode_number) {
                (Some(s), Some(e)) => format!("{t} S{s:02}E{e:02}"),
                _ => t.clone(),
            };
        }
        if let Some(t) = &self.radarr_movie_title {
            return match self.year {
                Some(y) => format!("{t} ({y})"),
                None => t.clone(),
            };
        }
        self.file_name.clone()
    }
}

pub struct NewAnalysis<'a> {
    pub library_id: Option<i64>,
    pub path: &'a str,
    pub size: i64,
    pub mtime: i64,
    pub info: &'a MediaInfo,
    pub assessment: &'a Assessment,
    pub broken: Option<&'a str>,
}

pub async fn upsert_analysis<'e, E: SqliteExecutor<'e>>(ex: E, a: &NewAnalysis<'_>) -> sqlx::Result<()> {
    let info = a.info;
    let v = info.video.clone().unwrap_or_default();
    let file_name = std::path::Path::new(a.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| a.path.to_string());
    let mut audio_codecs: Vec<&str> = info.audio.iter().map(|t| t.codec.as_str()).collect();
    audio_codecs.dedup();
    let mut sub_formats: Vec<String> =
        info.subtitles.iter().map(|s| if s.embedded { s.format.clone() } else { format!("{} (external)", s.format) }).collect();
    sub_formats.sort();
    sub_formats.dedup();
    let status = if a.broken.is_some() { "broken" } else { "ok" };
    sqlx::query(
        "INSERT INTO media_files (library_id, path, file_name, size, mtime, duration, container, video_codec,
            video_codec_tag, codec_tag_ok, video_profile, bit_depth, width, height, frame_rate, video_bitrate,
            overall_bitrate, hdr, hdr_type, fast_start, audio_codecs, audio_tracks, subtitle_formats, subtitle_tracks,
            compat_score, compat_verdict, quality_score, quality_verdict, issues, status, status_reason,
            processing_since, analyzed_at, first_seen_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,NULL,?,?)
         ON CONFLICT(path) DO UPDATE SET
            library_id = COALESCE(excluded.library_id, media_files.library_id),
            file_name = excluded.file_name, size = excluded.size, mtime = excluded.mtime,
            duration = excluded.duration, container = excluded.container, video_codec = excluded.video_codec,
            video_codec_tag = excluded.video_codec_tag, codec_tag_ok = excluded.codec_tag_ok,
            video_profile = excluded.video_profile, bit_depth = excluded.bit_depth, width = excluded.width,
            height = excluded.height, frame_rate = excluded.frame_rate, video_bitrate = excluded.video_bitrate,
            overall_bitrate = excluded.overall_bitrate, hdr = excluded.hdr, hdr_type = excluded.hdr_type,
            fast_start = excluded.fast_start, audio_codecs = excluded.audio_codecs,
            audio_tracks = excluded.audio_tracks, subtitle_formats = excluded.subtitle_formats,
            subtitle_tracks = excluded.subtitle_tracks, compat_score = excluded.compat_score,
            compat_verdict = excluded.compat_verdict, quality_score = excluded.quality_score,
            quality_verdict = excluded.quality_verdict, issues = excluded.issues, status = excluded.status,
            status_reason = excluded.status_reason, processing_since = NULL, analyzed_at = excluded.analyzed_at",
    )
    .bind(a.library_id)
    .bind(a.path)
    .bind(file_name)
    .bind(a.size)
    .bind(a.mtime)
    .bind(info.duration)
    .bind(&info.container)
    .bind(&v.codec)
    .bind(&v.codec_tag)
    .bind(v.codec_tag.is_empty() || crate::media::rating::codec_tag_ok(&v.codec, &v.codec_tag))
    .bind(&v.profile)
    .bind(v.bit_depth as i64)
    .bind(v.width as i64)
    .bind(v.height as i64)
    .bind(v.frame_rate)
    .bind(crate::media::rating::video_bitrate(info) as i64)
    .bind(info.overall_bitrate as i64)
    .bind(v.hdr_type.is_some())
    .bind(v.hdr_type.clone().unwrap_or_default())
    .bind(info.fast_start.unwrap_or(false))
    .bind(audio_codecs.join(","))
    .bind(Json(&info.audio))
    .bind(sub_formats.join(","))
    .bind(Json(&info.subtitles))
    .bind(a.assessment.compat_score as i64)
    .bind(&a.assessment.compat_verdict)
    .bind(a.assessment.quality_score as i64)
    .bind(&a.assessment.quality_verdict)
    .bind(Json(&a.assessment.issues))
    .bind(status)
    .bind(a.broken)
    .bind(now())
    .bind(now())
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn update_scores<'e, E: SqliteExecutor<'e>>(ex: E, id: i64, a: &Assessment) -> sqlx::Result<()> {
    sqlx::query("UPDATE media_files SET compat_score=?, compat_verdict=?, quality_score=?, quality_verdict=?, issues=? WHERE id=?")
        .bind(a.compat_score as i64)
        .bind(&a.compat_verdict)
        .bind(a.quality_score as i64)
        .bind(&a.quality_verdict)
        .bind(Json(&a.issues))
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn get_media(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<MediaFile>> {
    sqlx::query_as("SELECT * FROM media_files WHERE id = ?").bind(id).fetch_optional(pool).await
}

pub async fn get_media_by_path(pool: &SqlitePool, path: &str) -> sqlx::Result<Option<MediaFile>> {
    sqlx::query_as("SELECT * FROM media_files WHERE path = ?").bind(path).fetch_optional(pool).await
}

pub async fn list_libraries(pool: &SqlitePool) -> sqlx::Result<Vec<LibraryWithStats>> {
    sqlx::query_as(
        "SELECT l.*,
            COUNT(m.id) AS file_count,
            COALESCE(SUM(m.size), 0) AS total_size,
            COALESCE(SUM(m.compat_verdict = 'optimal'), 0) AS optimal,
            COALESCE(SUM(m.compat_verdict = 'good'), 0) AS good,
            COALESCE(SUM(m.compat_verdict = 'poor'), 0) AS poor,
            COALESCE(SUM(m.status = 'broken'), 0) AS broken,
            AVG(CASE WHEN m.status != 'broken' THEN m.compat_score END) AS avg_compat,
            AVG(CASE WHEN m.status != 'broken' THEN m.quality_score END) AS avg_quality
         FROM libraries l LEFT JOIN media_files m ON m.library_id = l.id
         GROUP BY l.id ORDER BY l.name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_library(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Library>> {
    sqlx::query_as("SELECT * FROM libraries WHERE id = ?").bind(id).fetch_optional(pool).await
}

/// Library whose root contains `path` (longest root wins).
pub async fn library_for_path(pool: &SqlitePool, path: &str) -> sqlx::Result<Option<Library>> {
    let libs: Vec<Library> = sqlx::query_as("SELECT * FROM libraries").fetch_all(pool).await?;
    Ok(libs.into_iter().filter(|l| path_is_under(path, &l.path)).max_by_key(|l| l.path.len()))
}

pub fn path_is_under(path: &str, root: &str) -> bool {
    let root = root.trim_end_matches(['/', '\\']);
    if root.is_empty() {
        return path.starts_with('/');
    }
    path == root || (path.starts_with(root) && matches!(path.as_bytes().get(root.len()), Some(b'/') | Some(b'\\')))
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaQuery {
    pub search: Option<String>,
    pub library_id: Option<i64>,
    pub compat: Option<String>,
    pub quality: Option<String>,
    pub status: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub resolution: Option<String>,
    pub hdr: Option<String>,
    pub issue: Option<String>,
    pub servarr: Option<String>,
    pub min_compat: Option<i64>,
    pub max_compat: Option<i64>,
    pub min_quality: Option<i64>,
    pub max_quality: Option<i64>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

fn push_filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &MediaQuery) {
    qb.push(" WHERE 1=1");
    if let Some(s) = q.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", s.replace('%', "\\%").replace('_', "\\_"));
        qb.push(" AND (path LIKE ")
            .push_bind(like.clone())
            .push(" ESCAPE '\\' OR sonarr_series_title LIKE ")
            .push_bind(like.clone())
            .push(" ESCAPE '\\' OR radarr_movie_title LIKE ")
            .push_bind(like)
            .push(" ESCAPE '\\')");
    }
    if let Some(id) = q.library_id {
        qb.push(" AND library_id = ").push_bind(id);
    }
    for (col, val) in [
        ("compat_verdict", &q.compat),
        ("quality_verdict", &q.quality),
        ("status", &q.status),
        ("video_codec", &q.video_codec),
        ("container", &q.container),
        ("servarr_kind", &q.servarr),
    ] {
        if let Some(v) = val.as_ref().filter(|v| !v.is_empty()) {
            if v == "none" && col == "servarr_kind" {
                qb.push(" AND servarr_kind IS NULL");
                continue;
            }
            let values: Vec<&str> = v.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
            qb.push(format!(" AND {col} IN ("));
            let mut sep = qb.separated(", ");
            for val in values {
                sep.push_bind(val.to_string());
            }
            qb.push(")");
        }
    }
    if let Some(a) = q.audio_codec.as_ref().filter(|v| !v.is_empty()) {
        qb.push(" AND (',' || audio_codecs || ',') LIKE ").push_bind(format!("%,{a},%"));
    }
    if let Some(r) = q.resolution.as_deref() {
        let cond = match r {
            "4k" => Some("(width >= 3200 OR height >= 2000)"),
            "1080p" => Some("(width < 3200 AND height < 2000) AND (width >= 1800 OR height >= 1000)"),
            "720p" => Some("(width < 1800 AND height < 1000) AND (width >= 1200 OR height >= 700)"),
            "sd" => Some("(width < 1200 AND height < 700)"),
            _ => None,
        };
        if let Some(c) = cond {
            qb.push(" AND ").push(c);
        }
    }
    match q.hdr.as_deref() {
        Some("true") | Some("hdr") => {
            qb.push(" AND hdr = 1");
        }
        Some("false") | Some("sdr") => {
            qb.push(" AND hdr = 0");
        }
        Some(t) if !t.is_empty() => {
            qb.push(" AND hdr_type = ").push_bind(t.to_string());
        }
        _ => {}
    }
    if let Some(code) = q.issue.as_ref().filter(|v| !v.is_empty()) {
        qb.push(" AND EXISTS (SELECT 1 FROM json_each(media_files.issues) WHERE json_extract(value, '$.code') = ").push_bind(code.clone()).push(")");
    }
    for (col, op, val) in [
        ("compat_score", ">=", q.min_compat),
        ("compat_score", "<=", q.max_compat),
        ("quality_score", ">=", q.min_quality),
        ("quality_score", "<=", q.max_quality),
    ] {
        if let Some(v) = val {
            qb.push(format!(" AND {col} {op} ")).push_bind(v);
        }
    }
}

pub fn sort_clause(q: &MediaQuery) -> String {
    let col = match q.sort.as_deref().unwrap_or("name") {
        "size" => "size",
        "compat" => "compat_score",
        "quality" => "quality_score",
        "analyzed" => "analyzed_at",
        "duration" => "duration",
        "bitrate" => "overall_bitrate",
        "resolution" => "width * height",
        "codec" => "video_codec",
        "path" => "path",
        _ => "file_name COLLATE NOCASE",
    };
    let dir = if q.dir.as_deref() == Some("desc") { "DESC" } else { "ASC" };
    format!(" ORDER BY {col} {dir}, id {dir}")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub async fn query_media(pool: &SqlitePool, q: &MediaQuery) -> sqlx::Result<Page<MediaFile>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 500);

    let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM media_files");
    push_filters(&mut count, q);
    let total: i64 = count.build_query_scalar().fetch_one(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM media_files");
    push_filters(&mut qb, q);
    qb.push(sort_clause(q));
    qb.push(" LIMIT ").push_bind(page_size).push(" OFFSET ").push_bind((page - 1) * page_size);
    let items = qb.build_query_as::<MediaFile>().fetch_all(pool).await?;
    Ok(Page { items, total, page, page_size })
}

/// All ids matching a filter (for "select all across pages" bulk actions).
pub async fn query_media_ids(pool: &SqlitePool, q: &MediaQuery) -> sqlx::Result<Vec<i64>> {
    let mut qb = QueryBuilder::<Sqlite>::new("SELECT id FROM media_files");
    push_filters(&mut qb, q);
    qb.push(sort_clause(q));
    qb.build_query_scalar().fetch_all(pool).await
}

/// Stream of all rows matching a filter, for exports.
pub async fn query_media_all(pool: &SqlitePool, q: &MediaQuery) -> sqlx::Result<Vec<MediaFile>> {
    let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM media_files");
    push_filters(&mut qb, q);
    qb.push(sort_clause(q));
    qb.build_query_as::<MediaFile>().fetch_all(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_root() {
        assert!(path_is_under("/media/tv/a.mkv", "/media/tv"));
        assert!(path_is_under("/media/tv/a.mkv", "/media/tv/"));
        assert!(!path_is_under("/media/tv2/a.mkv", "/media/tv"));
        assert!(path_is_under("/media/tv", "/media/tv"));
    }
}
