//! Jellyfin playback history, read from Jellystat. Jellystat records every Jellyfin session with
//! its transcode details; plays are linked to analyzed files to show what actually transcodes.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::db::now;
use crate::integrations::servarr::norm_key;
use crate::jobs::{Event, JobHandle};
use crate::settings::{self, map_path, JellystatSettings, PathMapping};
use crate::state::AppState;

/// Activity rows per history page.
const PAGE_SIZE: u64 = 500;
/// Stop after this many pages so a misbehaving server cannot loop forever.
const MAX_PAGES: u64 = 2000;
/// Days of history each background sync re-reads, so plays that ended late are picked up.
const SYNC_DAYS: i64 = 2;

pub struct Client {
    http: reqwest::Client,
    pub settings: JellystatSettings,
}

impl Client {
    pub fn new(http: reqwest::Client, settings: JellystatSettings) -> Self {
        Self { http, settings }
    }

    pub async fn from_state(state: &AppState) -> Result<Self> {
        let s = load_settings(&state.pool).await?;
        if !s.enabled || s.base_url.trim().is_empty() || s.api_key.trim().is_empty() {
            bail!("Jellystat is not configured");
        }
        Ok(Self::new(state.http.clone(), s))
    }

    async fn get(&self, path: &str, query: &[(&str, String)], timeout: Duration) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/{}", self.settings.base_url.trim().trim_end_matches('/'), path.trim_start_matches('/')))
            .query(query)
            .header("x-api-token", self.settings.api_key.trim())
            .header("Accept", "application/json")
            // Jellystat never answers when a query fails on its side.
            .timeout(timeout)
            .send()
            .await
            .context("Jellystat unreachable")?;
        match resp.status() {
            s if s.is_success() => Ok(resp.json().await.context("Jellystat returned invalid JSON")?),
            s if s == reqwest::StatusCode::UNAUTHORIZED || s == reqwest::StatusCode::FORBIDDEN => bail!("Jellystat rejected the API key ({s})"),
            s => bail!("Jellystat returned {s} for {path}"),
        }
    }

    /// Checks the URL and API key; returns the number of plays Jellystat holds.
    pub async fn test(&self) -> Result<u64> {
        let v = self.get("api/getHistory", &[("size", "1".into()), ("page", "1".into())], Duration::from_secs(30)).await?;
        if v.get("results").is_none() {
            bail!("This does not look like Jellystat; check the URL");
        }
        Ok(v["pages"].as_u64().unwrap_or(0))
    }

    /// Every playback activity row started at or after `since` (RFC 3339, UTC).
    async fn history_since(&self, since: &str, mut progress: impl FnMut(usize)) -> Result<Vec<Value>> {
        let filters = json!([{ "field": "ActivityDateInserted", "min": since }]).to_string();
        let mut rows = Vec::new();
        let mut page = 1;
        loop {
            let query = [("size", PAGE_SIZE.to_string()), ("page", page.to_string()), ("filters", filters.clone())];
            let v = self.get("api/getHistory", &query, Duration::from_secs(120)).await?;
            // Jellystat groups plays by item and user; the single plays sit in each group's `results`,
            // including older plays of that group, which are dropped here.
            for group in v["results"].as_array().into_iter().flatten() {
                for row in group["results"].as_array().into_iter().flatten() {
                    if row["ActivityDateInserted"].as_str().and_then(utc).is_some_and(|t| t.as_str() >= since) {
                        rows.push(row.clone());
                    }
                }
            }
            progress(rows.len());
            if page >= v["pages"].as_u64().unwrap_or(0) || page >= MAX_PAGES {
                break;
            }
            page += 1;
        }
        Ok(rows)
    }

    /// File path and item type of a Jellyfin item, as Jellystat's library sync saw it.
    async fn item(&self, id: &str) -> Result<(Option<String>, Option<String>)> {
        let resp = self
            .http
            .post(format!("{}/api/getItemDetails", self.settings.base_url.trim().trim_end_matches('/')))
            .header("x-api-token", self.settings.api_key.trim())
            .json(&json!({ "Id": id }))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context("Jellystat unreachable")?;
        if !resp.status().is_success() {
            bail!("Jellystat returned {} for item {id}", resp.status());
        }
        let v: Value = resp.json().await?;
        let first = v.as_array().and_then(|a| a.first()).cloned().unwrap_or_default();
        let text = |k: &str| first[k].as_str().filter(|s| !s.is_empty()).map(String::from);
        Ok((text("Path"), text("Type")))
    }
}

/// Loads the Jellystat settings, carrying the path mappings over from the 2.x Jellyfin settings once.
pub async fn load_settings(pool: &sqlx::SqlitePool) -> Result<JellystatSettings> {
    if settings::exists(pool, "jellystat").await? {
        return settings::load(pool).await;
    }
    let old: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'jellyfin'").fetch_optional(pool).await?;
    let path_mappings = old
        .and_then(|(json,)| serde_json::from_str::<Value>(&json).ok())
        .and_then(|v| serde_json::from_value::<Vec<PathMapping>>(v["pathMappings"].clone()).ok())
        .unwrap_or_default();
    Ok(JellystatSettings { path_mappings, ..Default::default() })
}

/// Normalizes a Jellystat timestamp to the RFC 3339 UTC form Optimarr stores.
fn utc(raw: &str) -> Option<String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(raw)
        .or_else(|_| chrono::DateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f%#z"))
        .ok()?
        .with_timezone(&chrono::Utc);
    Some(parsed.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

/// Splits "Transcode (v:h264 a:aac)" into method and transcode target codecs.
pub fn parse_play_method(raw: &str) -> (String, Option<String>, Option<String>) {
    let lower = raw.to_ascii_lowercase();
    let method = if lower.starts_with("transcode") {
        "Transcode"
    } else if lower.starts_with("directstream") || lower.starts_with("direct stream") {
        "DirectStream"
    } else {
        "DirectPlay"
    };
    let grab = |prefix: &str| {
        lower
            .split(|c: char| c == '(' || c == ')' || c.is_whitespace())
            .find_map(|t| t.strip_prefix(prefix))
            .filter(|c| !c.is_empty() && *c != "direct")
            .map(str::to_string)
    };
    (method.into(), grab("v:"), grab("a:"))
}

/// What Jellyfin's `TranscodingInfo` says about a transcode.
#[derive(Debug, Default, PartialEq)]
struct Transcode {
    reasons: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    container: Option<String>,
    hw_accel: Option<String>,
}

impl Transcode {
    fn from_info(ti: &Value) -> Self {
        let text = |k: &str| ti[k].as_str().filter(|s| !s.is_empty()).map(String::from);
        Self {
            reasons: match &ti["TranscodeReasons"] {
                Value::Array(a) => Some(a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty()),
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            },
            video_codec: text("VideoCodec").filter(|_| ti["IsVideoDirect"].as_bool() != Some(true)),
            audio_codec: text("AudioCodec").filter(|_| ti["IsAudioDirect"].as_bool() != Some(true)),
            container: text("Container"),
            hw_accel: text("HardwareAccelerationType"),
        }
    }
}

/// One play from Jellystat, ready to store.
#[derive(Debug, PartialEq)]
struct HistoryRow {
    key: String,
    item_id: String,
    user_id: Option<String>,
    user_name: Option<String>,
    item_name: String,
    started_at: String,
    stopped_at: Option<String>,
    duration: Option<f64>,
    client: Option<String>,
    device_name: Option<String>,
    device_id: Option<String>,
    method: String,
    transcode: Transcode,
}

impl HistoryRow {
    fn parse(r: &Value) -> Option<Self> {
        let text = |k: &str| r[k].as_str().filter(|s| !s.is_empty()).map(String::from);
        let id = text("Id")?;
        // For episodes Jellystat stores the series as NowPlayingItemId and the episode as EpisodeId.
        let item_id = text("EpisodeId").or_else(|| text("NowPlayingItemId"))?;
        let started_at = utc(r["ActivityDateInserted"].as_str()?)?;
        let duration = r["PlaybackDuration"].as_f64().or_else(|| r["PlaybackDuration"].as_str()?.parse().ok());
        let stopped_at = duration.and_then(|d| {
            let start = chrono::DateTime::parse_from_rfc3339(&started_at).ok()?;
            Some((start + chrono::Duration::milliseconds((d * 1000.0) as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        });
        // Plays Jellystat imported from the Playback Reporting plugin say "Transcode (v:h264 a:aac)".
        let (method, v_codec, a_codec) = parse_play_method(&text("PlayMethod").unwrap_or_default());
        let mut transcode = if method == "Transcode" { Transcode::from_info(&r["TranscodingInfo"]) } else { Transcode::default() };
        transcode.video_codec = transcode.video_codec.or(v_codec);
        transcode.audio_codec = transcode.audio_codec.or(a_codec);
        Some(Self {
            key: format!("js:{id}"),
            item_id,
            user_id: text("UserId"),
            user_name: text("UserName"),
            item_name: text("NowPlayingItemName").unwrap_or_default(),
            started_at,
            stopped_at,
            duration,
            client: text("Client"),
            device_name: text("DeviceName"),
            device_id: text("DeviceId"),
            method,
            transcode,
        })
    }
}

struct Resolver {
    by_path: HashMap<String, (i64, Option<i64>)>,
    by_name: HashMap<String, (i64, Option<i64>)>,
}

impl Resolver {
    async fn load(state: &AppState) -> Result<Self> {
        let rows: Vec<(i64, String, String, Option<i64>)> =
            sqlx::query_as("SELECT id, path, file_name, library_id FROM media_files").fetch_all(&state.pool).await?;
        let mut by_path = HashMap::with_capacity(rows.len());
        let mut by_name = HashMap::with_capacity(rows.len());
        for (id, path, name, lib) in rows {
            by_path.insert(norm_key(&path), (id, lib));
            by_name.insert(name.to_lowercase(), (id, lib));
        }
        Ok(Self { by_path, by_name })
    }

    fn resolve(&self, mappings: &[PathMapping], remote: &str) -> (String, Option<i64>, Option<i64>) {
        if remote.is_empty() {
            return (String::new(), None, None);
        }
        let local = map_path(mappings, remote);
        let hit = self.by_path.get(&norm_key(&local)).or_else(|| {
            let name = std::path::Path::new(&local).file_name()?.to_string_lossy().to_lowercase();
            self.by_name.get(&name)
        });
        (local, hit.map(|h| h.0), hit.and_then(|h| h.1))
    }
}

/// Import history from Jellystat for the last `days` days.
pub async fn start_history_import(state: &AppState, days: i64) -> Result<i64> {
    if let Some(j) = state.jobs.find_running("playback_sync", None) {
        return Ok(j.id);
    }
    let job = state.jobs.start("playback_sync", "Import playback history from Jellystat", None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = import_history(&st, &job, days).await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(id)
}

/// Remote path and type per item: what earlier imports already resolved, the rest from Jellystat.
async fn item_info(state: &AppState, client: &Client, ids: &[String], job: &JobHandle) -> Result<HashMap<String, (Option<String>, Option<String>)>> {
    let mut out = HashMap::new();
    let known: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT jellyfin_item_id, MAX(remote_path), MAX(item_type) FROM playback_events
         WHERE remote_path IS NOT NULL AND remote_path != '' GROUP BY jellyfin_item_id",
    )
    .fetch_all(&state.pool)
    .await?;
    for (id, path, kind) in known {
        out.insert(id, (Some(path), kind));
    }
    let missing: Vec<&String> = ids.iter().filter(|id| !out.contains_key(*id)).collect();
    for (i, id) in missing.iter().enumerate() {
        job.set_message(format!("Looking up files in Jellystat ({}/{})", i + 1, missing.len()));
        match client.item(id).await {
            Ok(info) => {
                out.insert((*id).clone(), info);
            }
            Err(e) => tracing::debug!(item = %id, error = %format!("{e:#}"), "jellystat item lookup failed"),
        }
    }
    Ok(out)
}

async fn import_history(state: &AppState, job: &JobHandle, days: i64) -> Result<()> {
    let client = Client::from_state(state).await?;
    let since = (chrono::Utc::now() - chrono::Duration::days(days.clamp(1, 3650))).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    job.set_message("Reading history from Jellystat");
    let raw = client.history_since(&since, |n| job.set_message(format!("Read {n} plays from Jellystat"))).await?;
    let rows: Vec<HistoryRow> = raw.iter().filter_map(HistoryRow::parse).collect();
    job.set_total(rows.len() as i64);

    let mut item_ids: Vec<String> = rows.iter().map(|r| r.item_id.clone()).collect();
    item_ids.sort();
    item_ids.dedup();
    let items = item_info(state, &client, &item_ids, job).await?;
    let resolver = Resolver::load(state).await?;
    job.set_message(format!("Saving {} plays", rows.len()));

    let mut tx = state.pool.begin().await?;
    let (mut inserted, mut merged) = (0u64, 0u64);
    for r in &rows {
        let (remote_path, item_type) = items.get(&r.item_id).cloned().unwrap_or_default();
        let remote_path = remote_path.unwrap_or_default();
        let (local, media_id, lib_id) = resolver.resolve(&client.settings.path_mappings, &remote_path);
        let t = &r.transcode;

        // Optimarr 2.0 and 2.1 sampled live Jellyfin sessions; fill those plays in instead of
        // recording them twice.
        let live: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM playback_events
             WHERE source_key LIKE 'live:%' AND jellyfin_item_id = ?
               AND (user_id IS NULL OR ? IS NULL OR user_id = ?) AND (device_id IS NULL OR ? IS NULL OR device_id = ?)
               AND ABS(julianday(started_at) - julianday(?)) * 1440 <= 10
             ORDER BY ABS(julianday(started_at) - julianday(?)) LIMIT 1",
        )
        .bind(&r.item_id)
        .bind(&r.user_id)
        .bind(&r.user_id)
        .bind(&r.device_id)
        .bind(&r.device_id)
        .bind(&r.started_at)
        .bind(&r.started_at)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((id,)) = live {
            sqlx::query(
                "UPDATE playback_events SET user_name = COALESCE(user_name, ?), item_type = COALESCE(item_type, ?),
                    client_name = COALESCE(client_name, ?), device_name = COALESCE(device_name, ?),
                    file_path = CASE WHEN file_path = '' THEN ? ELSE file_path END, remote_path = COALESCE(NULLIF(remote_path, ''), ?),
                    media_file_id = COALESCE(media_file_id, ?), library_id = COALESCE(library_id, ?),
                    duration_seconds = MAX(COALESCE(duration_seconds, 0), COALESCE(?, 0)),
                    play_method = CASE WHEN play_method = 'Transcode' OR ? = 'Transcode' THEN 'Transcode' ELSE play_method END,
                    transcode_reasons = COALESCE(transcode_reasons, ?), transcode_video_codec = COALESCE(transcode_video_codec, ?),
                    transcode_audio_codec = COALESCE(transcode_audio_codec, ?), transcode_container = COALESCE(transcode_container, ?),
                    hw_accel = COALESCE(hw_accel, ?), synced_at = ?
                 WHERE id = ?",
            )
            .bind(&r.user_name)
            .bind(&item_type)
            .bind(&r.client)
            .bind(&r.device_name)
            .bind(&local)
            .bind(&remote_path)
            .bind(media_id)
            .bind(lib_id)
            .bind(r.duration)
            .bind(&r.method)
            .bind(&t.reasons)
            .bind(&t.video_codec)
            .bind(&t.audio_codec)
            .bind(&t.container)
            .bind(&t.hw_accel)
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
            merged += 1;
            job.inc_processed();
            continue;
        }

        let res = sqlx::query(
            "INSERT INTO playback_events (source_key, jellyfin_item_id, user_id, user_name, item_name, item_type, file_path,
                remote_path, started_at, stopped_at, duration_seconds, client_name, device_name, device_id, play_method,
                transcode_reasons, transcode_video_codec, transcode_audio_codec, transcode_container, hw_accel, synced_at,
                media_file_id, library_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(source_key) DO UPDATE SET user_name = excluded.user_name, item_type = excluded.item_type,
                stopped_at = excluded.stopped_at, duration_seconds = excluded.duration_seconds, play_method = excluded.play_method,
                transcode_reasons = excluded.transcode_reasons, transcode_video_codec = excluded.transcode_video_codec,
                transcode_audio_codec = excluded.transcode_audio_codec, transcode_container = excluded.transcode_container,
                hw_accel = excluded.hw_accel, synced_at = excluded.synced_at, file_path = excluded.file_path,
                remote_path = excluded.remote_path, media_file_id = excluded.media_file_id, library_id = excluded.library_id",
        )
        .bind(&r.key)
        .bind(&r.item_id)
        .bind(&r.user_id)
        .bind(&r.user_name)
        .bind(&r.item_name)
        .bind(&item_type)
        .bind(&local)
        .bind(&remote_path)
        .bind(&r.started_at)
        .bind(&r.stopped_at)
        .bind(r.duration)
        .bind(&r.client)
        .bind(&r.device_name)
        .bind(&r.device_id)
        .bind(&r.method)
        .bind(&t.reasons)
        .bind(&t.video_codec)
        .bind(&t.audio_codec)
        .bind(&t.container)
        .bind(&t.hw_accel)
        .bind(now())
        .bind(media_id)
        .bind(lib_id)
        .execute(&mut *tx)
        .await?;
        inserted += res.rows_affected();
        job.inc_processed();
    }
    tx.commit().await?;
    job.set_message(if merged > 0 {
        format!("Imported {} from Jellystat and filled in {} recorded earlier", plays(inserted), plays(merged))
    } else {
        format!("Imported {} from Jellystat", plays(inserted))
    });
    Ok(())
}

fn plays(n: u64) -> String {
    if n == 1 {
        "1 play".into()
    } else {
        format!("{n} plays")
    }
}

/// Background loop: import recent Jellystat history on the configured interval.
pub fn spawn_background(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        let mut last_import: Option<std::time::Instant> = None;
        loop {
            tick.tick().await;
            let Ok(s) = load_settings(&state.pool).await else { continue };
            if !s.enabled || s.sync_interval_minutes == 0 {
                continue;
            }
            if last_import.is_none_or(|t| t.elapsed().as_secs() >= s.sync_interval_minutes * 60) {
                last_import = Some(std::time::Instant::now());
                if let Err(e) = start_history_import(&state, SYNC_DAYS).await {
                    tracing::debug!(error = %format!("{e:#}"), "jellystat sync failed to start");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_methods() {
        assert_eq!(parse_play_method("Transcode (v:h264 a:aac)"), ("Transcode".into(), Some("h264".into()), Some("aac".into())));
        assert_eq!(parse_play_method("Transcode (v:direct a:aac)"), ("Transcode".into(), None, Some("aac".into())));
        assert_eq!(parse_play_method("DirectPlay").0, "DirectPlay");
        assert_eq!(parse_play_method("DirectStream").0, "DirectStream");
    }

    #[test]
    fn timestamps() {
        assert_eq!(utc("2026-10-01T20:15:00.5+02:00").as_deref(), Some("2026-10-01T18:15:00.500Z"));
        assert_eq!(utc("2026-10-01 20:15:00.123+00").as_deref(), Some("2026-10-01T20:15:00.123Z"));
        assert_eq!(utc("garbage"), None);
    }

    #[test]
    fn parses_episode_transcode() {
        let row = json!({
            "Id": "abc", "UserId": "u1", "UserName": "anna", "Client": "Jellyfin Web", "DeviceName": "Firefox", "DeviceId": "d1",
            "NowPlayingItemId": "series", "EpisodeId": "episode", "NowPlayingItemName": "Pilot", "PlaybackDuration": 1500,
            "ActivityDateInserted": "2026-10-01T20:00:00.000+02:00", "PlayMethod": "Transcode",
            "TranscodingInfo": { "TranscodeReasons": ["AudioCodecNotSupported"], "IsVideoDirect": true, "VideoCodec": "hevc",
                "IsAudioDirect": false, "AudioCodec": "aac", "Container": "ts", "HardwareAccelerationType": "vaapi" }
        });
        let r = HistoryRow::parse(&row).unwrap();
        assert_eq!(r.key, "js:abc");
        assert_eq!(r.item_id, "episode");
        assert_eq!(r.started_at, "2026-10-01T18:00:00.000Z");
        assert_eq!(r.stopped_at.as_deref(), Some("2026-10-01T18:25:00.000Z"));
        assert_eq!(r.method, "Transcode");
        assert_eq!(r.transcode.reasons.as_deref(), Some("AudioCodecNotSupported"));
        assert_eq!(r.transcode.video_codec, None);
        assert_eq!(r.transcode.audio_codec.as_deref(), Some("aac"));
        assert_eq!(r.transcode.hw_accel.as_deref(), Some("vaapi"));
    }

    #[test]
    fn parses_plugin_import_and_direct_play() {
        let plugin = json!({ "Id": "1", "NowPlayingItemId": "movie", "ActivityDateInserted": "2026-10-01T20:00:00Z",
            "PlayMethod": "Transcode (v:h264 a:direct)", "PlaybackDuration": "60" });
        let r = HistoryRow::parse(&plugin).unwrap();
        assert_eq!((r.item_id.as_str(), r.transcode.video_codec.as_deref(), r.transcode.audio_codec.as_deref()), ("movie", Some("h264"), None));
        assert_eq!(r.duration, Some(60.0));
        let direct = json!({ "Id": "2", "NowPlayingItemId": "movie", "ActivityDateInserted": "2026-10-01T20:00:00Z",
            "PlayMethod": "DirectPlay", "TranscodingInfo": { "TranscodeReasons": ["ContainerNotSupported"] } });
        assert_eq!(HistoryRow::parse(&direct).unwrap().transcode, Transcode::default());
        assert!(HistoryRow::parse(&json!({ "Id": "3" })).is_none());
    }
}
