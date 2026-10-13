use serde::{Deserialize, Serialize};

/// Technical description of a media file, produced by a probe backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub container: String,
    pub duration: f64,
    pub overall_bitrate: u64,
    /// MP4 family only: whether the moov atom precedes mdat. `None` when not applicable.
    #[serde(default)]
    pub fast_start: Option<bool>,
    pub video: Option<VideoStream>,
    pub audio: Vec<AudioTrack>,
    pub subtitles: Vec<SubtitleTrack>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoStream {
    pub codec: String,
    pub codec_tag: String,
    pub profile: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub bit_depth: u32,
    pub bitrate: u64,
    pub hdr_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub codec: String,
    pub channels: u32,
    pub sample_rate: u32,
    pub bitrate: u64,
    pub language: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub format: String,
    pub language: String,
    pub embedded: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl SubtitleTrack {
    /// Image based subtitles cannot be converted to text without OCR.
    pub fn is_image_based(&self) -> bool {
        matches!(self.format.as_str(), "PGS" | "VobSub" | "DVB")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssueArea {
    Compatibility,
    Quality,
}

/// A single finding about a file, with the score impact and how to fix it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub area: IssueArea,
    pub severity: Severity,
    pub message: String,
    pub penalty: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub compat_score: i32,
    pub compat_verdict: String,
    pub quality_score: i32,
    pub quality_verdict: String,
    pub issues: Vec<Issue>,
    /// Single ffmpeg command that resolves every fixable compatibility issue, if any.
    pub ffmpeg_command: Option<String>,
    /// One sentence a human (or agent) can read.
    pub summary: String,
}
