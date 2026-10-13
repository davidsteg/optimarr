use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite};

use super::ApiResult;
use crate::repo::Page;
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackQuery {
    pub search: Option<String>,
    pub method: Option<String>,
    pub user: Option<String>,
    pub client: Option<String>,
    pub media_id: Option<i64>,
    pub library_id: Option<i64>,
    pub days: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackEvent {
    pub id: i64,
    pub jellyfin_item_id: String,
    pub user_name: Option<String>,
    pub item_name: String,
    pub item_type: Option<String>,
    pub file_path: String,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub client_name: Option<String>,
    pub device_name: Option<String>,
    pub play_method: String,
    pub transcode_reasons: Option<String>,
    pub transcode_video_codec: Option<String>,
    pub transcode_audio_codec: Option<String>,
    pub hw_accel: Option<String>,
    pub media_file_id: Option<i64>,
    pub library_id: Option<i64>,
    pub compat_verdict: Option<String>,
    pub compat_score: Option<i64>,
}

fn filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &PlaybackQuery) {
    qb.push(" WHERE 1=1");
    if let Some(s) = q.search.as_ref().filter(|s| !s.is_empty()) {
        qb.push(" AND (p.item_name LIKE ").push_bind(format!("%{s}%")).push(" OR p.file_path LIKE ").push_bind(format!("%{s}%")).push(")");
    }
    if let Some(m) = q.method.as_ref().filter(|s| !s.is_empty()) {
        qb.push(" AND p.play_method = ").push_bind(m.clone());
    }
    if let Some(u) = q.user.as_ref().filter(|s| !s.is_empty()) {
        qb.push(" AND p.user_name = ").push_bind(u.clone());
    }
    if let Some(c) = q.client.as_ref().filter(|s| !s.is_empty()) {
        qb.push(" AND (p.client_name = ").push_bind(c.clone()).push(" OR p.device_name = ").push_bind(c.clone()).push(")");
    }
    if let Some(id) = q.media_id {
        qb.push(" AND p.media_file_id = ").push_bind(id);
    }
    if let Some(id) = q.library_id {
        qb.push(" AND p.library_id = ").push_bind(id);
    }
    if let Some(d) = q.days.filter(|d| *d > 0) {
        qb.push(" AND p.started_at >= ").push_bind((chrono::Utc::now() - chrono::Duration::days(d)).to_rfc3339());
    }
}

pub async fn list(State(state): State<AppState>, Query(q): Query<PlaybackQuery>) -> ApiResult<Json<Page<PlaybackEvent>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 500);
    let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM playback_events p");
    filters(&mut count, &q);
    let total: i64 = count.build_query_scalar().fetch_one(&state.pool).await?;
    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT p.*, m.compat_verdict, m.compat_score FROM playback_events p LEFT JOIN media_files m ON m.id = p.media_file_id",
    );
    filters(&mut qb, &q);
    qb.push(" ORDER BY p.started_at DESC LIMIT ").push_bind(page_size).push(" OFFSET ").push_bind((page - 1) * page_size);
    let items = qb.build_query_as::<PlaybackEvent>().fetch_all(&state.pool).await?;
    Ok(Json(Page { items, total, page, page_size }))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Group {
    key: Option<String>,
    plays: i64,
    transcodes: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct TopFile {
    media_file_id: i64,
    title: String,
    plays: i64,
    transcodes: i64,
    compat_verdict: String,
    compat_score: i64,
}

pub async fn stats(State(state): State<AppState>, Query(q): Query<PlaybackQuery>) -> ApiResult<Json<Value>> {
    let since = (chrono::Utc::now() - chrono::Duration::days(q.days.unwrap_or(30).clamp(1, 3650))).to_rfc3339();
    let group = |col: &str| {
        format!(
            "SELECT {col} AS key, COUNT(*) AS plays, COALESCE(SUM(play_method = 'Transcode'), 0) AS transcodes
             FROM playback_events WHERE started_at >= ? GROUP BY 1 ORDER BY 2 DESC LIMIT 20"
        )
    };
    let by_method: Vec<Group> = sqlx::query_as(&group("play_method")).bind(&since).fetch_all(&state.pool).await?;
    let by_client: Vec<Group> = sqlx::query_as(&group("COALESCE(client_name, 'Unknown')")).bind(&since).fetch_all(&state.pool).await?;
    let by_user: Vec<Group> = sqlx::query_as(&group("COALESCE(user_name, 'Unknown')")).bind(&since).fetch_all(&state.pool).await?;
    let by_reason: Vec<Group> = sqlx::query_as(
        "WITH RECURSIVE split(word, rest) AS (
            SELECT '', transcode_reasons || ',' FROM playback_events WHERE started_at >= ? AND transcode_reasons IS NOT NULL
            UNION ALL SELECT trim(substr(rest, 0, instr(rest, ','))), substr(rest, instr(rest, ',') + 1) FROM split WHERE rest != '')
         SELECT word AS key, COUNT(*) AS plays, COUNT(*) AS transcodes FROM split WHERE word != '' GROUP BY 1 ORDER BY 2 DESC LIMIT 20",
    )
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    let top_transcoded: Vec<TopFile> = sqlx::query_as(
        "SELECT m.id AS media_file_id,
                COALESCE(m.radarr_movie_title, m.sonarr_series_title, m.file_name) AS title,
                COUNT(*) AS plays, SUM(p.play_method = 'Transcode') AS transcodes, m.compat_verdict, m.compat_score
         FROM playback_events p JOIN media_files m ON m.id = p.media_file_id
         WHERE p.started_at >= ? GROUP BY m.id HAVING transcodes > 0 ORDER BY transcodes DESC, plays DESC LIMIT 20",
    )
    .bind(&since)
    .fetch_all(&state.pool)
    .await?;
    let (plays, transcodes, unmatched): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(play_method='Transcode'),0), COALESCE(SUM(media_file_id IS NULL),0) FROM playback_events WHERE started_at >= ?",
    )
    .bind(&since)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({
        "plays": plays,
        "transcodes": transcodes,
        "transcodeRate": if plays > 0 { transcodes as f64 / plays as f64 } else { 0.0 },
        "unmatched": unmatched,
        "byMethod": by_method,
        "byClient": by_client,
        "byUser": by_user,
        "byReason": by_reason,
        "topTranscoded": top_transcoded,
    })))
}
