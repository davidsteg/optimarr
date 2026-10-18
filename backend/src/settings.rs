//! Typed application settings, persisted as JSON documents in the `settings` table.

use std::collections::BTreeMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::SqlitePool;

/// What the target playback clients can direct-play, and how much each problem costs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CompatibilityProfile {
    /// Keys are codec names, optionally suffixed with bit depth, e.g. "H.265 10-bit".
    pub video_codecs: BTreeMap<String, bool>,
    pub audio_codecs: BTreeMap<String, bool>,
    pub containers: BTreeMap<String, bool>,
    pub subtitle_formats: BTreeMap<String, bool>,
    pub bit_depths: BTreeMap<String, bool>,
    pub weights: CompatibilityWeights,
    pub thresholds: CompatibilityThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CompatibilityWeights {
    pub unsupported_video_codec: i32,
    pub unsupported_audio_codec: i32,
    pub unsupported_container: i32,
    pub unsupported_subtitle_format: i32,
    pub unsupported_bit_depth: i32,
    pub incorrect_codec_tag: i32,
    pub missing_fast_start: i32,
    pub high_bitrate: i32,
    pub high_bitrate_threshold_mbps: f64,
}

impl Default for CompatibilityWeights {
    fn default() -> Self {
        Self {
            unsupported_video_codec: 35,
            unsupported_audio_codec: 25,
            unsupported_container: 30,
            unsupported_subtitle_format: 8,
            unsupported_bit_depth: 18,
            incorrect_codec_tag: 12,
            missing_fast_start: 5,
            high_bitrate: 5,
            high_bitrate_threshold_mbps: 40.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CompatibilityThresholds {
    pub optimal: i32,
    pub good: i32,
}

impl Default for CompatibilityThresholds {
    fn default() -> Self {
        Self { optimal: 80, good: 60 }
    }
}

fn map(items: &[(&str, bool)]) -> BTreeMap<String, bool> {
    items.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

impl Default for CompatibilityProfile {
    fn default() -> Self {
        Self {
            video_codecs: map(&[
                ("H.264", true),
                ("H.265", true),
                ("H.265 10-bit", false),
                ("VP9", true),
                ("AV1", false),
                ("MPEG-4 Visual", false),
                ("MPEG-2", false),
                ("VC-1", false),
            ]),
            audio_codecs: map(&[
                ("AAC", true),
                ("MP3", true),
                ("AC3", false),
                ("EAC3", false),
                ("DTS", false),
                ("TrueHD", false),
                ("FLAC", true),
                ("Opus", true),
                ("Vorbis", true),
                ("ALAC", true),
                ("PCM", false),
            ]),
            containers: map(&[("MP4", true), ("MKV", false), ("WebM", true), ("TS", false), ("AVI", false)]),
            subtitle_formats: map(&[
                ("SRT", true),
                ("VTT", true),
                ("MOV_TEXT", true),
                ("ASS", false),
                ("SSA", false),
                ("PGS", false),
                ("VobSub", false),
            ]),
            bit_depths: map(&[("8", true), ("10", false), ("12", false)]),
            weights: CompatibilityWeights::default(),
            thresholds: CompatibilityThresholds::default(),
        }
    }
}

/// Thresholds for the quality score. Everything is tunable from the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct QualityProfile {
    /// Penalty applied when the resolution is below each tier.
    pub penalty_1080p: i32,
    pub penalty_720p: i32,
    pub penalty_sd: i32,
    pub penalty_low_bitrate: i32,
    pub penalty_very_low_bitrate: i32,
    pub penalty_legacy_codec: i32,
    pub penalty_stereo_only: i32,
    pub penalty_low_audio_bitrate: i32,
    pub penalty_sdr_4k: i32,
    /// Minimum bits per pixel per frame before a file is considered starved (H.264 baseline;
    /// modern codecs get 60% of this).
    pub min_bits_per_pixel: f64,
    /// Languages every file should have an audio track in (any one of them). Empty disables the check.
    pub preferred_audio_languages: Vec<String>,
    /// Languages every file should have subtitles in (any one of them). Empty disables the check.
    pub preferred_subtitle_languages: Vec<String>,
    pub penalty_missing_audio_language: i32,
    pub penalty_missing_subtitle_language: i32,
    /// Applied when the only subtitles in a preferred language are image based (PGS, VobSub).
    pub penalty_image_subtitles: i32,
    pub thresholds: QualityThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct QualityThresholds {
    pub excellent: i32,
    pub good: i32,
    pub fair: i32,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self { excellent: 85, good: 70, fair: 50 }
    }
}

impl Default for QualityProfile {
    fn default() -> Self {
        Self {
            penalty_1080p: 10,
            penalty_720p: 25,
            penalty_sd: 45,
            penalty_low_bitrate: 15,
            penalty_very_low_bitrate: 30,
            penalty_legacy_codec: 10,
            penalty_stereo_only: 5,
            penalty_low_audio_bitrate: 5,
            penalty_sdr_4k: 5,
            min_bits_per_pixel: 0.06,
            preferred_audio_languages: Vec::new(),
            preferred_subtitle_languages: Vec::new(),
            penalty_missing_audio_language: 15,
            penalty_missing_subtitle_language: 5,
            penalty_image_subtitles: 5,
            thresholds: QualityThresholds::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PathMapping {
    /// Path as Sonarr/Radarr/Jellyfin sees it.
    pub remote: String,
    /// Path as Optimarr sees it.
    pub local: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ServarrSettings {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub basic_auth_username: Option<String>,
    pub basic_auth_password: Option<String>,
    pub path_mappings: Vec<PathMapping>,
}

/// Jellystat, the source of Jellyfin playback history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct JellystatSettings {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    /// Maps paths as Jellyfin sees them to paths as Optimarr sees them.
    pub path_mappings: Vec<PathMapping>,
    /// Background sync interval. 0 disables the background sync.
    pub sync_interval_minutes: u64,
}

impl Default for JellystatSettings {
    fn default() -> Self {
        Self { enabled: false, base_url: String::new(), api_key: String::new(), path_mappings: Vec::new(), sync_interval_minutes: 5 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    /// Concurrent probe processes during scans. 0 = number of CPUs.
    pub scan_concurrency: usize,
    /// Watch enabled libraries for new or changed files.
    pub watch_libraries: bool,
    /// Rescan files marked "processing" (redownload requested) after this many hours.
    pub processing_rescan_hours: i64,
    /// Preferred probe backend: auto | mediainfo | ffprobe.
    pub probe_backend: String,
    /// Incremental rescan of every enabled library every N hours (catches changes on network
    /// mounts where file watching does not work). 0 disables.
    pub periodic_scan_hours: u64,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self { scan_concurrency: 0, watch_libraries: true, processing_rescan_hours: 24, probe_backend: "auto".into(), periodic_scan_hours: 12 }
    }
}

/// Key under which each settings document is stored.
pub trait SettingsDoc: Serialize + DeserializeOwned + Default + Clone {
    const KEY: &'static str;
}

impl SettingsDoc for CompatibilityProfile {
    const KEY: &'static str = "compatibility";
}
impl SettingsDoc for QualityProfile {
    const KEY: &'static str = "quality";
}
impl SettingsDoc for GeneralSettings {
    const KEY: &'static str = "general";
}
impl SettingsDoc for JellystatSettings {
    const KEY: &'static str = "jellystat";
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SonarrSettings(pub ServarrSettings);
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RadarrSettings(pub ServarrSettings);

impl SettingsDoc for SonarrSettings {
    const KEY: &'static str = "sonarr";
}
impl SettingsDoc for RadarrSettings {
    const KEY: &'static str = "radarr";
}

pub async fn load<T: SettingsDoc>(pool: &SqlitePool) -> anyhow::Result<T> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?").bind(T::KEY).fetch_optional(pool).await?;
    Ok(match row {
        Some((json,)) => serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::warn!(key = T::KEY, error = %e, "invalid settings document, using defaults");
            T::default()
        }),
        None => T::default(),
    })
}

pub async fn save<T: SettingsDoc>(pool: &SqlitePool, value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string(value)?;
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(T::KEY)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn exists(pool: &SqlitePool, key: &str) -> anyhow::Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM settings WHERE key = ?").bind(key).fetch_optional(pool).await?;
    Ok(row.is_some())
}

/// Apply path mappings: replace the longest matching remote prefix with its local counterpart.
pub fn map_path(mappings: &[PathMapping], remote_path: &str) -> String {
    let norm = remote_path.replace('\\', "/");
    let mut best: Option<&PathMapping> = None;
    for m in mappings {
        let remote = m.remote.replace('\\', "/");
        let remote = remote.trim_end_matches('/');
        if remote.is_empty() {
            continue;
        }
        let matches = norm == remote || norm.strip_prefix(remote).is_some_and(|rest| rest.starts_with('/'));
        if matches && best.is_none_or(|b| b.remote.len() < m.remote.len()) {
            best = Some(m);
        }
    }
    match best {
        Some(m) => {
            let remote = m.remote.replace('\\', "/");
            let rest = &norm[remote.trim_end_matches('/').len()..];
            format!("{}{}", m.local.trim_end_matches('/'), rest)
        }
        None => norm,
    }
}

/// Inverse of [`map_path`]: translate a local path to the remote view.
pub fn unmap_path(mappings: &[PathMapping], local_path: &str) -> String {
    let swapped: Vec<PathMapping> = mappings.iter().map(|m| PathMapping { remote: m.local.clone(), local: m.remote.clone() }).collect();
    map_path(&swapped, local_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_longest_prefix() {
        let m = vec![PathMapping { remote: "/data".into(), local: "/media".into() }, PathMapping { remote: "/data/tv".into(), local: "/tv".into() }];
        assert_eq!(map_path(&m, "/data/tv/Show/S01E01.mkv"), "/tv/Show/S01E01.mkv");
        assert_eq!(map_path(&m, "/data/movies/a.mkv"), "/media/movies/a.mkv");
        assert_eq!(map_path(&m, "/database/a.mkv"), "/database/a.mkv");
        assert_eq!(map_path(&m, "D:\\x\\y"), "D:/x/y");
        assert_eq!(unmap_path(&m, "/tv/Show/a.mkv"), "/data/tv/Show/a.mkv");
    }
}
