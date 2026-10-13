//! Per-device playback profiles: what each of your clients can direct-play, and what Jellyfin
//! would have to do with a given file on that client.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::SqlitePool;

use crate::db::now;
use crate::media::model::MediaInfo;
use crate::media::rating::{audio_codec_supported, container_supported, subtitle_supported, video_codec_supported};
use crate::settings::CompatibilityProfile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceCapabilities {
    pub video_codecs: BTreeMap<String, bool>,
    pub audio_codecs: BTreeMap<String, bool>,
    pub containers: BTreeMap<String, bool>,
    pub subtitle_formats: BTreeMap<String, bool>,
    pub bit_depths: BTreeMap<String, bool>,
    /// Can display HDR. Without it, HDR video is tone-mapped, which means a transcode.
    pub hdr: bool,
    /// Widest video the device plays (0 = no limit), e.g. 1920 for a 1080p TV.
    pub max_width: u32,
    /// Highest bitrate the device or its network link handles (0 = no limit).
    pub max_bitrate_mbps: f64,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        let c = CompatibilityProfile::default();
        Self {
            video_codecs: c.video_codecs,
            audio_codecs: c.audio_codecs,
            containers: c.containers,
            subtitle_formats: c.subtitle_formats,
            bit_depths: c.bit_depths,
            hdr: false,
            max_width: 0,
            max_bitrate_mbps: 0.0,
        }
    }
}

impl DeviceCapabilities {
    fn as_compat(&self) -> CompatibilityProfile {
        CompatibilityProfile {
            video_codecs: self.video_codecs.clone(),
            audio_codecs: self.audio_codecs.clone(),
            containers: self.containers.clone(),
            subtitle_formats: self.subtitle_formats.clone(),
            bit_depths: self.bit_depths.clone(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub profile: Json<DeviceCapabilities>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    DirectPlay,
    /// Container is remuxed on the fly; cheap, video and audio untouched.
    DirectStream,
    Transcode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub outcome: Outcome,
    pub reasons: Vec<String>,
    /// Subtitle formats that force a transcode only when turned on.
    pub subtitle_note: Option<String>,
}

pub fn evaluate(info: &MediaInfo, caps: &DeviceCapabilities) -> Verdict {
    let p = caps.as_compat();
    let mut transcode = Vec::new();
    let mut remux = Vec::new();
    match &info.video {
        None => transcode.push("No readable video stream".to_string()),
        Some(v) => {
            if !video_codec_supported(&p, &v.codec, v.bit_depth) {
                transcode.push(format!("{} {}-bit video", v.codec, v.bit_depth));
            } else if !p.bit_depths.get(&v.bit_depth.to_string()).copied().unwrap_or(false) {
                transcode.push(format!("{}-bit colour", v.bit_depth));
            }
            if let Some(h) = v.hdr_type.as_ref().filter(|_| !caps.hdr) {
                transcode.push(format!("{h} needs tone mapping"));
            }
            if caps.max_width > 0 && v.width > caps.max_width {
                transcode.push(format!("{}px wide is above the {}px limit", v.width, caps.max_width));
            }
        }
    }
    if caps.max_bitrate_mbps > 0.0 && info.overall_bitrate as f64 / 1e6 > caps.max_bitrate_mbps {
        transcode.push(format!("{:.1} Mbps is above the {:.0} Mbps limit", info.overall_bitrate as f64 / 1e6, caps.max_bitrate_mbps));
    }
    if !info.audio.is_empty() && !info.audio.iter().any(|a| audio_codec_supported(&p, &a.codec)) {
        let mut codecs: Vec<&str> = info.audio.iter().map(|a| a.codec.as_str()).collect();
        codecs.dedup();
        transcode.push(format!("{} audio", codecs.join(", ")));
    }
    if !info.container.is_empty() && !container_supported(&p, &info.container) {
        remux.push(format!("{} container", info.container));
    }
    let mut bad_subs: Vec<&str> = info.subtitles.iter().filter(|s| !subtitle_supported(&p, &s.format)).map(|s| s.format.as_str()).collect();
    bad_subs.sort();
    bad_subs.dedup();
    let subtitle_note = (!bad_subs.is_empty()).then(|| format!("{} subtitles are burned in (transcode) when turned on", bad_subs.join(", ")));

    let outcome = if !transcode.is_empty() {
        Outcome::Transcode
    } else if !remux.is_empty() {
        Outcome::DirectStream
    } else {
        Outcome::DirectPlay
    };
    transcode.extend(remux);
    Verdict { outcome, reasons: transcode, subtitle_note }
}

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<Device>> {
    sqlx::query_as("SELECT * FROM device_profiles ORDER BY name").fetch_all(pool).await
}

pub async fn list_enabled(pool: &SqlitePool) -> sqlx::Result<Vec<Device>> {
    sqlx::query_as("SELECT * FROM device_profiles WHERE enabled = 1 ORDER BY name").fetch_all(pool).await
}

pub async fn get(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Device>> {
    sqlx::query_as("SELECT * FROM device_profiles WHERE id = ?").bind(id).fetch_optional(pool).await
}

pub async fn create(pool: &SqlitePool, name: &str, enabled: bool, caps: &DeviceCapabilities) -> sqlx::Result<i64> {
    sqlx::query_scalar("INSERT INTO device_profiles (name, enabled, profile, created_at) VALUES (?, ?, ?, ?) RETURNING id")
        .bind(name)
        .bind(enabled)
        .bind(Json(caps))
        .bind(now())
        .fetch_one(pool)
        .await
}

pub async fn update(pool: &SqlitePool, id: i64, name: &str, enabled: bool, caps: &DeviceCapabilities) -> sqlx::Result<bool> {
    let r = sqlx::query("UPDATE device_profiles SET name = ?, enabled = ?, profile = ? WHERE id = ?")
        .bind(name)
        .bind(enabled)
        .bind(Json(caps))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> sqlx::Result<bool> {
    Ok(sqlx::query("DELETE FROM device_profiles WHERE id = ?").bind(id).execute(pool).await?.rows_affected() > 0)
}

fn caps(video: &[&str], audio: &[&str], containers: &[&str], subs: &[&str], depths: &[&str], hdr: bool) -> DeviceCapabilities {
    let base = DeviceCapabilities::default();
    let pick = |all: &BTreeMap<String, bool>, on: &[&str]| -> BTreeMap<String, bool> {
        let mut m: BTreeMap<String, bool> = all.keys().map(|k| (k.clone(), on.contains(&k.as_str()))).collect();
        for k in on {
            m.insert(k.to_string(), true);
        }
        m
    };
    DeviceCapabilities {
        video_codecs: pick(&base.video_codecs, video),
        audio_codecs: pick(&base.audio_codecs, audio),
        containers: pick(&base.containers, containers),
        subtitle_formats: pick(&base.subtitle_formats, subs),
        bit_depths: pick(&base.bit_depths, depths),
        hdr,
        max_width: 0,
        max_bitrate_mbps: 0.0,
    }
}

/// Starting points for common clients. Support varies by model and app version, so these are
/// meant to be adjusted.
pub fn templates() -> Vec<(&'static str, DeviceCapabilities)> {
    vec![
        ("Web browser", caps(&["H.264", "VP9", "AV1"], &["AAC", "MP3", "Opus", "FLAC", "Vorbis"], &["MP4", "WebM"], &["SRT", "VTT"], &["8"], false)),
        (
            "Apple TV / iPhone",
            caps(
                &["H.264", "H.265", "H.265 10-bit"],
                &["AAC", "AC3", "EAC3", "ALAC", "FLAC", "MP3"],
                &["MP4"],
                &["SRT", "VTT", "MOV_TEXT"],
                &["8", "10"],
                true,
            ),
        ),
        (
            "Android TV / Shield",
            caps(
                &["H.264", "H.265", "H.265 10-bit", "VP9", "MPEG-2"],
                &["AAC", "AC3", "EAC3", "DTS", "TrueHD", "FLAC", "Opus", "MP3", "Vorbis"],
                &["MKV", "MP4", "WebM", "TS"],
                &["SRT", "VTT", "ASS", "SSA", "PGS"],
                &["8", "10"],
                true,
            ),
        ),
        (
            "Smart TV (LG / Samsung)",
            caps(
                &["H.264", "H.265", "H.265 10-bit", "VP9"],
                &["AAC", "AC3", "EAC3", "MP3", "FLAC"],
                &["MKV", "MP4", "TS"],
                &["SRT"],
                &["8", "10"],
                true,
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::model::{AudioTrack, SubtitleTrack, VideoStream};

    fn file(codec: &str, depth: u32, container: &str, audio: &str) -> MediaInfo {
        MediaInfo {
            container: container.into(),
            duration: 100.0,
            overall_bitrate: 8_000_000,
            video: Some(VideoStream { codec: codec.into(), bit_depth: depth, width: 1920, height: 1080, ..Default::default() }),
            audio: vec![AudioTrack { codec: audio.into(), channels: 6, ..Default::default() }],
            subtitles: vec![SubtitleTrack { format: "PGS".into(), embedded: true, ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn outcomes_per_device() {
        let t = templates();
        let browser = &t[0].1;
        let shield = &t[2].1;
        let hevc = file("H.265", 10, "MKV", "DTS");
        let v = evaluate(&hevc, browser);
        assert_eq!(v.outcome, Outcome::Transcode);
        assert!(v.reasons.iter().any(|r| r.contains("H.265")));
        assert!(v.subtitle_note.is_some());
        let v = evaluate(&hevc, shield);
        assert_eq!(v.outcome, Outcome::DirectPlay, "{v:?}");
        assert!(v.subtitle_note.is_none());

        let remux = file("H.264", 8, "MKV", "AAC");
        assert_eq!(evaluate(&remux, browser).outcome, Outcome::DirectStream);
    }

    #[test]
    fn limits_force_transcode() {
        let mut c = templates()[2].1.clone();
        c.max_bitrate_mbps = 5.0;
        assert_eq!(evaluate(&file("H.264", 8, "MKV", "AAC"), &c).outcome, Outcome::Transcode);
    }
}
