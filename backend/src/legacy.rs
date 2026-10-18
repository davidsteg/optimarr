//! One-time import from Optimarr 1.x: `data/optimarr.db` and `config/appsettings.json`.

use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

use crate::db::now;
use crate::media::files::mtime_secs;
use crate::media::model::*;
use crate::media::probe::{normalize_audio_codec, normalize_subtitle_format, normalize_video_codec};
use crate::media::rating;
use crate::repo::{self, NewAnalysis};
use crate::settings::*;

const MARKER: &str = "legacy_import";

pub async fn import_if_needed(pool: &SqlitePool, data_dir: &Path, config_dir: &Path) -> Result<()> {
    if exists(pool, MARKER).await? {
        return Ok(());
    }
    let mut summary = serde_json::Map::new();
    for candidate in [config_dir.join("appsettings.json"), config_dir.parent().unwrap_or(config_dir).join("appsettings.json")] {
        if candidate.exists() {
            match import_settings(pool, &candidate).await {
                Ok(()) => {
                    tracing::info!(file = %candidate.display(), "imported settings from Optimarr 1.x");
                    summary.insert("settings".into(), Value::String(candidate.display().to_string()));
                }
                Err(e) => tracing::warn!(error = %format!("{e:#}"), "could not import legacy settings"),
            }
            break;
        }
    }
    let legacy_db = data_dir.join("optimarr.db");
    if legacy_db.exists() {
        match import_db(pool, &legacy_db).await {
            Ok((libs, media, plays)) => {
                tracing::info!(libs, media, plays, "imported database from Optimarr 1.x");
                summary.insert("libraries".into(), libs.into());
                summary.insert("media".into(), media.into());
                summary.insert("playback".into(), plays.into());
            }
            Err(e) => tracing::warn!(error = %format!("{e:#}"), "could not import legacy database"),
        }
    }
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(MARKER)
        .bind(Value::Object(summary).to_string())
        .execute(pool)
        .await?;
    Ok(())
}

fn bool_map(v: &Value) -> Option<std::collections::BTreeMap<String, bool>> {
    let obj = v.as_object()?;
    let m: std::collections::BTreeMap<String, bool> = obj
        .iter()
        .filter_map(|(k, v)| {
            let b = v.as_bool().or_else(|| v.as_str().and_then(|s| s.parse().ok()))?;
            Some((k.clone(), b))
        })
        .collect();
    (!m.is_empty()).then_some(m)
}

fn int(v: &Value, k: &str) -> Option<i32> {
    v.get(k).and_then(|x| x.as_i64().or_else(|| x.as_str()?.parse().ok())).map(|x| x as i32)
}

async fn import_settings(pool: &SqlitePool, path: &Path) -> Result<()> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;

    if let Some(mp) = v.get("MediaPropertySettings") {
        let mut p = CompatibilityProfile::default();
        // 1.x lists keys like "H.265 8-bit"; keep them, they still work with the lookup rules.
        if let Some(m) = mp.get("VideoCodecs").and_then(bool_map) {
            p.video_codecs.extend(m);
        }
        if let Some(m) = mp.get("AudioCodecs").and_then(bool_map) {
            p.audio_codecs.extend(m);
        }
        if let Some(m) = mp.get("Containers").and_then(bool_map) {
            p.containers.extend(m);
        }
        if let Some(m) = mp.get("SubtitleFormats").and_then(bool_map) {
            p.subtitle_formats.extend(m);
        }
        if let Some(m) = mp.get("BitDepths").and_then(bool_map) {
            p.bit_depths.extend(m);
        }
        if let Some(w) = mp.get("RatingWeights") {
            let d = &mut p.weights;
            d.unsupported_video_codec = int(w, "UnsupportedVideoCodec").unwrap_or(d.unsupported_video_codec);
            d.unsupported_audio_codec = int(w, "UnsupportedAudioCodec").unwrap_or(d.unsupported_audio_codec);
            d.unsupported_container = int(w, "UnsupportedContainer").unwrap_or(d.unsupported_container);
            d.unsupported_subtitle_format = int(w, "UnsupportedSubtitleFormat").unwrap_or(d.unsupported_subtitle_format);
            d.unsupported_bit_depth = int(w, "UnsupportedBitDepth").unwrap_or(d.unsupported_bit_depth);
            d.incorrect_codec_tag = int(w, "IncorrectCodecTag").unwrap_or(d.incorrect_codec_tag);
            d.missing_fast_start = int(w, "FastStart").unwrap_or(d.missing_fast_start);
            d.high_bitrate = int(w, "HighBitrate").unwrap_or(d.high_bitrate);
            if let Some(t) = w.get("HighBitrateThresholdMbps").and_then(|x| x.as_f64()) {
                d.high_bitrate_threshold_mbps = t;
            }
        }
        if let Some(t) = mp.get("RatingThresholds") {
            p.thresholds.optimal = int(t, "Optimal").unwrap_or(p.thresholds.optimal);
            p.thresholds.good = int(t, "Good").unwrap_or(p.thresholds.good);
        }
        save(pool, &p).await?;
    }

    let servarr = |sec: &Value| -> ServarrSettings {
        let s = |k: &str| sec.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let opt = |k: &str| sec.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(String::from);
        ServarrSettings {
            enabled: sec.get("Enabled").and_then(Value::as_bool).unwrap_or(false),
            base_url: s("BaseUrl"),
            api_key: s("ApiKey"),
            basic_auth_username: opt("BasicAuthUsername"),
            basic_auth_password: opt("BasicAuthPassword"),
            path_mappings: sec
                .get("PathMappings")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|m| {
                            let from = m.get("From").or_else(|| m.get("from"))?.as_str()?;
                            let to = m.get("To").or_else(|| m.get("to"))?.as_str()?;
                            (!from.is_empty() && !to.is_empty()).then(|| PathMapping { remote: from.into(), local: to.into() })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    };
    if let Some(s) = v.pointer("/Servarr/Sonarr") {
        save(pool, &SonarrSettings(servarr(s))).await?;
    }
    if let Some(s) = v.pointer("/Servarr/Radarr") {
        save(pool, &RadarrSettings(servarr(s))).await?;
    }
    if let Some(key) = v.pointer("/Auth/ApiKey").and_then(Value::as_str).filter(|k| !k.is_empty()) {
        crate::auth::store_key(pool, key).await?;
    }
    Ok(())
}

fn get_s(r: &SqliteRow, c: &str) -> String {
    r.try_get::<Option<String>, _>(c).ok().flatten().unwrap_or_default()
}
fn get_i(r: &SqliteRow, c: &str) -> Option<i64> {
    r.try_get::<Option<i64>, _>(c).ok().flatten()
}
fn get_f(r: &SqliteRow, c: &str) -> f64 {
    r.try_get::<Option<f64>, _>(c).ok().flatten().unwrap_or(0.0)
}
fn get_b(r: &SqliteRow, c: &str) -> bool {
    get_i(r, c).unwrap_or(0) != 0
}

/// 1.x stored dates without a timezone ("2025-01-01 10:00:00" or with a T); they are UTC.
fn date(s: &str) -> String {
    if s.is_empty() {
        return now();
    }
    let s = s.replace(' ', "T");
    if s.ends_with('Z') || s.contains('+') {
        s
    } else {
        format!("{s}Z")
    }
}

async fn import_db(pool: &SqlitePool, path: &Path) -> Result<(usize, usize, usize)> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?.read_only(true);
    let old = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'").fetch_all(&old).await?;
    let has = |t: &str| tables.iter().any(|x| x == t);

    let mut lib_map = std::collections::HashMap::new();
    if has("LibraryPaths") {
        for r in sqlx::query("SELECT * FROM LibraryPaths").fetch_all(&old).await? {
            let category = match get_s(&r, "Category").to_lowercase().as_str() {
                c if c.contains("movie") => "movies",
                c if c.contains("tv") || c.contains("show") || c.contains("series") => "series",
                _ => "other",
            };
            let servarr = get_s(&r, "ServarrType").to_lowercase();
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO libraries (name, path, category, enabled, servarr_kind, servarr_root_folder_id, servarr_root_folder_path, created_at, last_scanned_at, last_synced_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(path) DO UPDATE SET name = excluded.name RETURNING id",
            )
            .bind(get_s(&r, "Name"))
            .bind(get_s(&r, "Path").trim_end_matches('/'))
            .bind(category)
            .bind(get_i(&r, "IsActive").unwrap_or(1) != 0)
            .bind((!servarr.is_empty()).then_some(servarr))
            .bind(get_i(&r, "ServarrRootFolderId"))
            .bind(Some(get_s(&r, "ServarrRootFolderPath")).filter(|s| !s.is_empty()))
            .bind(date(&get_s(&r, "CreatedAt")))
            .bind(Some(get_s(&r, "LastScannedAt")).filter(|s| !s.is_empty()).map(|s| date(&s)))
            .bind(Some(get_s(&r, "LastSyncedAt")).filter(|s| !s.is_empty()).map(|s| date(&s)))
            .fetch_one(pool)
            .await?;
            lib_map.insert(get_i(&r, "Id").unwrap_or_default(), id);
        }
    }

    let compat: CompatibilityProfile = load(pool).await?;
    let quality: QualityProfile = load(pool).await?;
    let libraries: Vec<repo::Library> = sqlx::query_as("SELECT * FROM libraries").fetch_all(pool).await?;
    let mut media = 0;
    let mut id_map = std::collections::HashMap::new();
    if has("VideoAnalyses") {
        let rows = sqlx::query("SELECT * FROM VideoAnalyses").fetch_all(&old).await?;
        let mut tx = pool.begin().await?;
        for r in &rows {
            let path = get_s(r, "FilePath");
            if path.is_empty() {
                continue;
            }
            let audio: Vec<AudioTrack> = serde_json::from_str::<Vec<Value>>(&get_s(r, "AudioTracksJson"))
                .unwrap_or_default()
                .iter()
                .map(|a| AudioTrack {
                    codec: normalize_audio_codec(a["codec"].as_str().unwrap_or("")),
                    channels: a["channels"].as_u64().unwrap_or(2) as u32,
                    sample_rate: a["sampleRate"].as_u64().unwrap_or(0) as u32,
                    bitrate: a["bitrate"].as_u64().unwrap_or(0) * 1000,
                    language: a["language"].as_str().unwrap_or("").into(),
                    ..Default::default()
                })
                .collect();
            let subtitles: Vec<SubtitleTrack> = serde_json::from_str::<Vec<Value>>(&get_s(r, "SubtitleTracksJson"))
                .unwrap_or_default()
                .iter()
                .map(|s| SubtitleTrack {
                    format: normalize_subtitle_format(s["format"].as_str().unwrap_or("")),
                    language: s["language"].as_str().unwrap_or("").into(),
                    embedded: s["isEmbedded"].as_bool().unwrap_or(true),
                    forced: false,
                    path: s["filePath"].as_str().filter(|p| !p.is_empty()).map(String::from),
                })
                .collect();
            let codec = get_s(r, "VideoCodec");
            let container = match get_s(r, "Container").to_uppercase().as_str() {
                "MATROSKA" => "MKV".to_string(),
                "MPEG-4" | "M4V" | "MOV" => "MP4".to_string(),
                "WEBM" => "WebM".to_string(),
                other => other.to_string(),
            };
            let broken = get_b(r, "IsBroken");
            let info = MediaInfo {
                fast_start: rating::is_mp4_family(&container).then_some(get_b(r, "IsFastStart")),
                container,
                duration: get_f(r, "Duration"),
                overall_bitrate: 0,
                video: (!codec.is_empty() && codec != "Unknown").then(|| VideoStream {
                    codec: normalize_video_codec(&codec),
                    codec_tag: get_s(r, "VideoCodecTag"),
                    profile: String::new(),
                    width: get_i(r, "Width").unwrap_or(0) as u32,
                    height: get_i(r, "Height").unwrap_or(0) as u32,
                    frame_rate: get_f(r, "FrameRate"),
                    bit_depth: get_i(r, "BitDepth").unwrap_or(8) as u32,
                    bitrate: 0,
                    hdr_type: get_b(r, "IsHDR").then(|| Some(get_s(r, "HDRType")).filter(|s| !s.is_empty()).unwrap_or("HDR10".into())),
                }),
                audio,
                subtitles,
            };
            let size = get_i(r, "FileSize").unwrap_or(0);
            let (size, mtime, exists) = match std::fs::metadata(&path) {
                Ok(md) => (md.len() as i64, mtime_secs(&md), true),
                Err(_) => (size, 0, false),
            };
            let a = rating::assess(&info, size as u64, &path, &compat, &quality);
            let lib = libraries.iter().filter(|l| repo::path_is_under(&path, &l.path)).max_by_key(|l| l.path.len()).map(|l| l.id);
            let reason = get_s(r, "BrokenReason");
            repo::upsert_analysis(
                &mut *tx,
                &NewAnalysis {
                    library_id: lib,
                    path: &path,
                    size,
                    mtime,
                    info: &info,
                    assessment: &a,
                    broken: broken.then_some(if reason.is_empty() { "Broken in Optimarr 1.x" } else { reason.as_str() }),
                },
            )
            .await?;
            let new_id: i64 = sqlx::query_scalar("SELECT id FROM media_files WHERE path = ?").bind(&path).fetch_one(&mut *tx).await?;
            let servarr = get_s(r, "ServarrType").to_lowercase();
            sqlx::query(
                "UPDATE media_files SET status = CASE WHEN ? THEN status ELSE 'missing' END,
                    servarr_kind=?, sonarr_series_id=?, sonarr_series_title=?, sonarr_episode_id=?, season_number=?,
                    episode_number=?, radarr_movie_id=?, radarr_movie_title=?, year=? WHERE id=?",
            )
            .bind(exists)
            .bind((!servarr.is_empty()).then_some(servarr))
            .bind(get_i(r, "SonarrSeriesId"))
            .bind(Some(get_s(r, "SonarrSeriesTitle")).filter(|s| !s.is_empty()))
            .bind(get_i(r, "SonarrEpisodeId"))
            .bind(get_i(r, "SonarrSeasonNumber"))
            .bind(get_i(r, "SonarrEpisodeNumber"))
            .bind(get_i(r, "RadarrMovieId"))
            .bind(Some(get_s(r, "RadarrMovieTitle")).filter(|s| !s.is_empty()))
            .bind(get_i(r, "RadarrYear"))
            .bind(new_id)
            .execute(&mut *tx)
            .await?;
            id_map.insert(get_i(r, "Id").unwrap_or_default(), new_id);
            media += 1;
        }
        tx.commit().await?;
    }

    let mut plays = 0;
    if has("PlaybackHistories") {
        let rows = sqlx::query("SELECT * FROM PlaybackHistories").fetch_all(&old).await?;
        let mut tx = pool.begin().await?;
        for r in &rows {
            let method = get_s(r, "PlayMethod");
            let (method, _, _) = crate::integrations::jellystat::parse_play_method(&method);
            sqlx::query(
                "INSERT OR IGNORE INTO playback_events (source_key, jellyfin_item_id, user_id, user_name, item_name, item_type, file_path,
                    started_at, stopped_at, client_name, device_name, device_id, play_method, transcode_reasons, transcode_video_codec,
                    transcode_audio_codec, transcode_container, hw_accel, synced_at, media_file_id, library_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("v1:{}", get_i(r, "Id").unwrap_or_default()))
            .bind(get_s(r, "JellyfinItemId"))
            .bind(Some(get_s(r, "JellyfinUserId")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "JellyfinUserName")).filter(|s| !s.is_empty()))
            .bind(get_s(r, "ItemName"))
            .bind(Some(get_s(r, "ItemType")).filter(|s| !s.is_empty()))
            .bind(get_s(r, "FilePath"))
            .bind(date(&get_s(r, "PlaybackStartTime")))
            .bind(Some(get_s(r, "PlaybackStopTime")).filter(|s| !s.is_empty()).map(|s| date(&s)))
            .bind(Some(get_s(r, "ClientName")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "DeviceName")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "DeviceId")).filter(|s| !s.is_empty()))
            .bind(method)
            .bind(Some(get_s(r, "TranscodeReason")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "TranscodeVideoCodec")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "TranscodeAudioCodec")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "TranscodeContainer")).filter(|s| !s.is_empty()))
            .bind(Some(get_s(r, "HardwareAccelerationType")).filter(|s| !s.is_empty()))
            .bind(date(&get_s(r, "SyncedAt")))
            .bind(get_i(r, "VideoAnalysisId").and_then(|i| id_map.get(&i).copied()))
            .bind(get_i(r, "LibraryPathId").and_then(|i| lib_map.get(&i).copied()))
            .execute(&mut *tx)
            .await?;
            plays += 1;
        }
        tx.commit().await?;
    }
    old.close().await;
    Ok((lib_map.len(), media, plays))
}
