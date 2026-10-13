//! Jellyfin playback history: live session sampling plus bulk import from the
//! Playback Reporting plugin. Events are linked to analyzed files to show what actually transcodes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::db::now;
use crate::integrations::servarr::norm_key;
use crate::jobs::{Event, JobHandle};
use crate::settings::{self, map_path, JellyfinSettings};
use crate::state::AppState;

pub struct Client {
    http: reqwest::Client,
    pub settings: JellyfinSettings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub server_name: String,
    pub version: String,
    pub playback_reporting: bool,
}

impl Client {
    pub fn new(http: reqwest::Client, settings: JellyfinSettings) -> Self {
        Self { http, settings }
    }

    pub async fn from_state(state: &AppState) -> Result<Self> {
        let s: JellyfinSettings = settings::load(&state.pool).await?;
        if !s.enabled || s.base_url.trim().is_empty() || s.api_key.trim().is_empty() {
            bail!("Jellyfin is not configured");
        }
        Ok(Self::new(state.http.clone(), s))
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let key = self.settings.api_key.trim();
        self.http
            .request(method, format!("{}/{}", self.settings.base_url.trim_end_matches('/'), path.trim_start_matches('/')))
            .header("Authorization", format!("MediaBrowser Token=\"{key}\", Client=\"Optimarr\""))
            .header("X-Emby-Token", key)
            .header("Accept", "application/json")
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self.req(reqwest::Method::GET, path).send().await.context("Jellyfin unreachable")?;
        if !resp.status().is_success() {
            bail!("Jellyfin returned {} for {path}", resp.status());
        }
        Ok(resp.json().await?)
    }

    pub async fn info(&self) -> Result<ServerInfo> {
        let v = self.get("System/Info").await?;
        Ok(ServerInfo {
            server_name: v["ServerName"].as_str().unwrap_or("Jellyfin").into(),
            version: v["Version"].as_str().unwrap_or("?").into(),
            playback_reporting: self.custom_query("SELECT 1").await.is_ok(),
        })
    }

    /// Run a read-only query against the Playback Reporting plugin's database.
    async fn custom_query(&self, sql: &str) -> Result<Vec<HashMap<String, Value>>> {
        let resp = self
            .req(reqwest::Method::POST, "user_usage_stats/submit_custom_query")
            .json(&json!({"CustomQueryString": sql, "ReplaceUserId": true}))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("Playback Reporting plugin unavailable ({})", resp.status());
        }
        let v: Value = resp.json().await?;
        let cols: Vec<String> = v
            .get("colums")
            .or_else(|| v.get("columns"))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|c| c.as_str().unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        let rows = v.get("results").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(rows.into_iter().filter_map(|r| r.as_array().cloned()).map(|r| cols.iter().cloned().zip(r).collect()).collect())
    }

    /// Item id -> file path, in batches.
    async fn item_paths(&self, ids: &[String]) -> Result<HashMap<String, String>> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let v = self.get(&format!("Items?Ids={}&Fields=Path&Recursive=true&EnableImages=false&EnableUserData=false", chunk.join(","))).await?;
            for item in v["Items"].as_array().into_iter().flatten() {
                if let (Some(id), Some(path)) = (item["Id"].as_str(), item["Path"].as_str()) {
                    out.insert(id.to_string(), path.to_string());
                }
            }
        }
        Ok(out)
    }
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

    fn resolve(&self, mappings: &[settings::PathMapping], remote: &str) -> (String, Option<i64>, Option<i64>) {
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

/// Import history from the Playback Reporting plugin for the last `days` days.
pub async fn start_history_import(state: &AppState, days: i64) -> Result<i64> {
    if let Some(j) = state.jobs.find_running("playback_sync", None) {
        return Ok(j.id);
    }
    let job = state.jobs.start("playback_sync", "Import Jellyfin playback history", None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = import_history(&st, &job, days).await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(id)
}

async fn import_history(state: &AppState, job: &JobHandle, days: i64) -> Result<()> {
    let client = Client::from_state(state).await?;
    let since = (chrono::Utc::now() - chrono::Duration::days(days.clamp(1, 3650))).format("%Y-%m-%d %H:%M:%S").to_string();
    let rows = client
        .custom_query(&format!(
            "SELECT rowid AS RowId, DateCreated, UserId, ItemId, ItemType, ItemName, PlaybackMethod, ClientName, DeviceName, PlayDuration \
             FROM PlaybackActivity WHERE DateCreated >= '{since}' ORDER BY DateCreated"
        ))
        .await
        .context("Install the Jellyfin \"Playback Reporting\" plugin to import history")?;
    job.set_total(rows.len() as i64);

    let mut item_ids: Vec<String> = rows.iter().filter_map(|r| r.get("ItemId")?.as_str().map(String::from)).collect();
    item_ids.sort();
    item_ids.dedup();
    job.set_message(format!("Resolving {} items", item_ids.len()));
    let paths = client.item_paths(&item_ids).await.unwrap_or_default();
    let resolver = Resolver::load(state).await?;

    let str_of = |r: &HashMap<String, Value>, k: &str| -> Option<String> {
        match r.get(k)? {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            v => Some(v.to_string()),
        }
    };
    let mut tx = state.pool.begin().await?;
    let mut inserted = 0;
    for r in &rows {
        let item_id = str_of(r, "ItemId").unwrap_or_default();
        let started = str_of(r, "DateCreated").unwrap_or_default();
        let user = str_of(r, "UserId");
        let raw_method = str_of(r, "PlaybackMethod").unwrap_or_default();
        let (method, v_codec, a_codec) = parse_play_method(&raw_method);
        let remote_path = paths.get(&item_id).cloned().unwrap_or_default();
        let (local, media_id, lib_id) = resolver.resolve(&client.settings.path_mappings, &remote_path);
        let key = format!("pr:{}", str_of(r, "RowId").unwrap_or_else(|| format!("{item_id}:{started}")));
        let duration: Option<f64> = str_of(r, "PlayDuration").and_then(|d| d.parse().ok());
        let res = sqlx::query(
            "INSERT INTO playback_events (source_key, jellyfin_item_id, user_name, item_name, item_type, file_path, started_at,
                duration_seconds, client_name, device_name, play_method, transcode_video_codec, transcode_audio_codec, synced_at,
                media_file_id, library_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(source_key) DO UPDATE SET media_file_id = excluded.media_file_id, library_id = excluded.library_id,
                file_path = excluded.file_path",
        )
        .bind(&key)
        .bind(&item_id)
        .bind(&user)
        .bind(str_of(r, "ItemName").unwrap_or_default())
        .bind(str_of(r, "ItemType"))
        .bind(&local)
        .bind(&started)
        .bind(duration)
        .bind(str_of(r, "ClientName"))
        .bind(str_of(r, "DeviceName"))
        .bind(&method)
        .bind(v_codec)
        .bind(a_codec)
        .bind(now())
        .bind(media_id)
        .bind(lib_id)
        .execute(&mut *tx)
        .await?;
        inserted += res.rows_affected();
        job.inc_processed();
    }
    tx.commit().await?;
    job.set_message(format!("Imported {inserted} playback events"));
    Ok(())
}

/// Tracks live sessions between polls.
#[derive(Default, Clone)]
pub struct SessionSampler {
    open: Arc<Mutex<HashMap<String, String>>>,
}

impl SessionSampler {
    /// Poll /Sessions once and record what is playing and how.
    pub async fn poll(&self, state: &AppState) -> Result<usize> {
        let client = Client::from_state(state).await?;
        let sessions = client.get("Sessions?ActiveWithinSeconds=300").await?;
        let empty = vec![];
        let sessions = sessions.as_array().unwrap_or(&empty);
        let mut active_keys = Vec::new();
        let mut need_paths = Vec::new();
        for s in sessions {
            let Some(item) = s.get("NowPlayingItem").filter(|i| i.is_object()) else { continue };
            if item["Path"].as_str().is_none() {
                if let Some(id) = item["Id"].as_str() {
                    need_paths.push(id.to_string());
                }
            }
        }
        let extra_paths = if need_paths.is_empty() { HashMap::new() } else { client.item_paths(&need_paths).await.unwrap_or_default() };
        let resolver = Resolver::load(state).await?;
        for s in sessions {
            let Some(item) = s.get("NowPlayingItem").filter(|i| i.is_object()) else { continue };
            let session_id = s["Id"].as_str().unwrap_or_default();
            let item_id = item["Id"].as_str().unwrap_or_default().to_string();
            let session_key = format!("{session_id}:{item_id}");
            active_keys.push(session_key.clone());
            let ps = &s["PlayState"];
            let ti = &s["TranscodingInfo"];
            let method = ps["PlayMethod"].as_str().unwrap_or("DirectPlay").to_string();
            let reasons = match &ti["TranscodeReasons"] {
                Value::Array(a) => Some(a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")),
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            };
            let remote = item["Path"].as_str().map(String::from).or_else(|| extra_paths.get(&item_id).cloned()).unwrap_or_default();
            let (local, media_id, lib_id) = resolver.resolve(&client.settings.path_mappings, &remote);
            let position = ps["PositionTicks"].as_i64().map(|t| t as f64 / 1e7);

            let existing = self.open.lock().unwrap().get(&session_key).cloned();
            let key = existing.unwrap_or_else(|| format!("live:{session_key}:{}", chrono::Utc::now().timestamp()));
            sqlx::query(
                "INSERT INTO playback_events (source_key, jellyfin_item_id, user_id, user_name, item_name, item_type, file_path,
                    started_at, stopped_at, duration_seconds, client_name, device_name, device_id, play_method, transcode_reasons,
                    transcode_video_codec, transcode_audio_codec, transcode_container, hw_accel, synced_at, media_file_id, library_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(source_key) DO UPDATE SET stopped_at = excluded.stopped_at, duration_seconds = excluded.duration_seconds,
                    play_method = CASE WHEN playback_events.play_method = 'Transcode' THEN 'Transcode' ELSE excluded.play_method END,
                    transcode_reasons = COALESCE(excluded.transcode_reasons, playback_events.transcode_reasons),
                    transcode_video_codec = COALESCE(excluded.transcode_video_codec, playback_events.transcode_video_codec),
                    transcode_audio_codec = COALESCE(excluded.transcode_audio_codec, playback_events.transcode_audio_codec),
                    hw_accel = COALESCE(excluded.hw_accel, playback_events.hw_accel), synced_at = excluded.synced_at",
            )
            .bind(&key)
            .bind(&item_id)
            .bind(s["UserId"].as_str())
            .bind(s["UserName"].as_str())
            .bind(item["Name"].as_str().unwrap_or_default())
            .bind(item["Type"].as_str())
            .bind(&local)
            .bind(now())
            .bind(now())
            .bind(position)
            .bind(s["Client"].as_str())
            .bind(s["DeviceName"].as_str())
            .bind(s["DeviceId"].as_str())
            .bind(&method)
            .bind(reasons)
            .bind(ti["VideoCodec"].as_str().filter(|_| ti["IsVideoDirect"].as_bool() != Some(true)))
            .bind(ti["AudioCodec"].as_str().filter(|_| ti["IsAudioDirect"].as_bool() != Some(true)))
            .bind(ti["Container"].as_str())
            .bind(ti["HardwareAccelerationType"].as_str())
            .bind(now())
            .bind(media_id)
            .bind(lib_id)
            .execute(&state.pool)
            .await?;
            // started_at must keep its first value; the upsert above only touches mutable columns.
            self.open.lock().unwrap().insert(session_key, key);
        }
        self.open.lock().unwrap().retain(|k, _| active_keys.contains(k));
        Ok(active_keys.len())
    }
}

/// Background loop: sample sessions every minute, import plugin history on the configured interval.
pub fn spawn_background(state: AppState) {
    tokio::spawn(async move {
        let sampler = SessionSampler::default();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        let mut last_import = std::time::Instant::now();
        loop {
            tick.tick().await;
            let Ok(s) = settings::load::<JellyfinSettings>(&state.pool).await else { continue };
            if !s.enabled {
                continue;
            }
            if let Err(e) = sampler.poll(&state).await {
                tracing::debug!(error = %format!("{e:#}"), "jellyfin session poll failed");
            }
            if s.sync_interval_minutes > 0 && last_import.elapsed().as_secs() >= s.sync_interval_minutes * 60 {
                last_import = std::time::Instant::now();
                let _ = start_history_import(&state, 2).await;
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
}
