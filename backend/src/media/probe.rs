//! Media probing through `mediainfo --Output=JSON` (preferred) or `ffprobe` (fallback).
//! Both are normalised into [`MediaInfo`].

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use tokio::process::Command;

use super::model::*;

const PROBE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    MediaInfo,
    FfProbe,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::MediaInfo => "mediainfo",
            Backend::FfProbe => "ffprobe",
        }
    }
}

/// Probe tools found on this machine.
#[derive(Debug, Clone, Default)]
pub struct Probers {
    pub mediainfo: Option<String>,
    pub ffprobe: Option<String>,
}

impl Probers {
    pub async fn detect() -> Self {
        async fn version(cmd: &str, arg: &str) -> Option<String> {
            let out = Command::new(cmd).arg(arg).output().await.ok()?;
            if !out.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.to_string())
        }
        let mediainfo = version("mediainfo", "--Version").await.map(|v| {
            v.trim_start_matches("MediaInfo Command line,").trim().to_string()
        });
        let mediainfo = match mediainfo {
            Some(v) if v.is_empty() => Some("mediainfo".into()),
            other => other,
        };
        let ffprobe = version("ffprobe", "-version").await;
        Self { mediainfo, ffprobe }
    }

    /// Ordered list of backends to try for the given preference (auto | mediainfo | ffprobe).
    pub fn order(&self, preference: &str) -> Vec<Backend> {
        let mut v = Vec::new();
        let mi = self.mediainfo.is_some();
        let ff = self.ffprobe.is_some();
        match preference {
            "ffprobe" => {
                if ff {
                    v.push(Backend::FfProbe)
                }
                if mi {
                    v.push(Backend::MediaInfo)
                }
            }
            _ => {
                if mi {
                    v.push(Backend::MediaInfo)
                }
                if ff {
                    v.push(Backend::FfProbe)
                }
            }
        }
        v
    }
}

pub async fn probe(path: &Path, backends: &[Backend]) -> Result<MediaInfo> {
    if backends.is_empty() {
        bail!("neither mediainfo nor ffprobe is installed");
    }
    let mut last_err = None;
    for b in backends {
        let res = match b {
            Backend::MediaInfo => probe_mediainfo(path).await,
            Backend::FfProbe => probe_ffprobe(path).await,
        };
        match res {
            Ok(info) if info.video.is_some() || !info.audio.is_empty() => return Ok(info),
            Ok(_) => last_err = Some(anyhow!("{} found no media streams", b.name())),
            Err(e) => last_err = Some(e.context(format!("{} failed", b.name()))),
        }
    }
    Err(last_err.unwrap())
}

async fn run(cmd: &mut Command) -> Result<Vec<u8>> {
    cmd.kill_on_drop(true).stdin(std::process::Stdio::null());
    let out = tokio::time::timeout(PROBE_TIMEOUT, cmd.output()).await.context("probe timed out")??;
    if !out.status.success() {
        bail!("exit {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(out.stdout)
}

pub async fn probe_mediainfo(path: &Path) -> Result<MediaInfo> {
    let out = run(Command::new("mediainfo").arg("--Output=JSON").arg("--Full").arg(path)).await?;
    let v: Value = serde_json::from_slice(&out).context("invalid mediainfo JSON")?;
    Ok(parse_mediainfo(&v, path))
}

pub async fn probe_ffprobe(path: &Path) -> Result<MediaInfo> {
    let out = run(Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path))
    .await?;
    let v: Value = serde_json::from_slice(&out).context("invalid ffprobe JSON")?;
    Ok(parse_ffprobe(&v, path))
}

// ---------------------------------------------------------------- helpers

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Numbers in mediainfo/ffprobe JSON are usually strings; some include units or separators.
fn num(v: &Value, key: &str) -> f64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(st)) => parse_num(st),
        _ => 0.0,
    }
}

fn parse_num(st: &str) -> f64 {
    let st = st.trim();
    if let Ok(f) = st.parse::<f64>() {
        return f;
    }
    // "24000/1001"
    if let Some((a, b)) = st.split_once('/') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            if b != 0.0 {
                return a / b;
            }
        }
    }
    // "1 920 pixels", "5 / 4"
    let cleaned: String = st
        .split(|c: char| c == '/' || c.is_alphabetic())
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    cleaned.parse().unwrap_or(0.0)
}

fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn container_from_extension(path: &Path) -> String {
    match ext_of(path).as_str() {
        "mp4" | "m4v" | "mov" => "MP4",
        "mkv" | "mka" => "MKV",
        "webm" => "WebM",
        "ts" | "m2ts" | "mts" => "TS",
        "avi" => "AVI",
        "ogg" | "ogv" => "OGG",
        "wmv" | "asf" => "ASF",
        "flv" => "FLV",
        "mpg" | "mpeg" | "vob" => "MPEG-PS",
        other => return other.to_ascii_uppercase(),
    }
    .to_string()
}

pub fn normalize_container(format: &str, path: &Path) -> String {
    let f = format.to_ascii_lowercase();
    if f.is_empty() {
        return container_from_extension(path);
    }
    if f.contains("webm") && !f.contains("matroska") {
        return "WebM".into();
    }
    if f.contains("matroska") {
        // ffprobe reports "matroska,webm" for both.
        return if ext_of(path) == "webm" { "WebM".into() } else { "MKV".into() };
    }
    if f.contains("mpeg-4") || f.contains("mp4") || f.contains("quicktime") || f.contains("mov") {
        return "MP4".into();
    }
    if f.contains("mpeg-ts") || f.contains("mpegts") || f.contains("bdav") {
        return "TS".into();
    }
    if f == "avi" {
        return "AVI".into();
    }
    if f.contains("mpeg-ps") || f == "mpeg" {
        return "MPEG-PS".into();
    }
    format.to_ascii_uppercase()
}

pub fn normalize_video_codec(format: &str) -> String {
    let u = format.to_ascii_uppercase();
    match () {
        _ if u.contains("AVC") || u == "H264" || u.contains("H.264") || u.contains("X264") => "H.264",
        _ if u.contains("HEVC") || u == "H265" || u.contains("H.265") || u.contains("X265") => "H.265",
        _ if u.contains("VP9") => "VP9",
        _ if u.contains("VP8") => "VP8",
        _ if u.contains("AV1") => "AV1",
        _ if u == "MPEG-4 VISUAL" || u == "MPEG4" || u.contains("XVID") || u.contains("DIVX") => "MPEG-4 Visual",
        _ if u == "MPEG VIDEO" || u.contains("MPEG2") || u == "MPEG-2" => "MPEG-2",
        _ if u.contains("VC-1") || u == "VC1" || u.contains("WMV3") => "VC-1",
        _ if u.contains("THEORA") => "Theora",
        _ if u.is_empty() => "Unknown",
        _ => return format.to_string(),
    }
    .to_string()
}

pub fn normalize_audio_codec(format: &str) -> String {
    let u = format.to_ascii_uppercase();
    match () {
        _ if u.contains("E-AC-3") || u == "EAC3" || u.contains("DD+") => "EAC3",
        _ if u.contains("AC-3") || u == "AC3" => "AC3",
        _ if u.contains("AAC") || u.contains("MP4A") => "AAC",
        _ if u.contains("MLP FBA") || u.contains("TRUEHD") => "TrueHD",
        _ if u.contains("DTS") => "DTS",
        _ if u.contains("FLAC") => "FLAC",
        _ if u.contains("OPUS") => "Opus",
        _ if u.contains("VORBIS") => "Vorbis",
        _ if u.contains("ALAC") => "ALAC",
        _ if u.starts_with("PCM") || u.contains("LPCM") => "PCM",
        _ if u.contains("MPEG AUDIO") || u == "MP3" || u.contains("LAYER 3") => "MP3",
        _ if u == "MP2" => "MP2",
        _ if u.is_empty() => "Unknown",
        _ => return format.to_string(),
    }
    .to_string()
}

pub fn normalize_subtitle_format(format: &str) -> String {
    let u = format.to_ascii_uppercase();
    match () {
        _ if u.contains("PGS") => "PGS",
        _ if u.contains("VOBSUB") || u.contains("DVD_SUBTITLE") || u == "DVDSUB" => "VobSub",
        _ if u.contains("DVB") => "DVB",
        _ if u == "UTF-8" || u.contains("SUBRIP") || u == "SRT" || u.contains("S_TEXT/UTF8") => "SRT",
        _ if u.contains("WEBVTT") || u == "VTT" => "VTT",
        _ if u == "ASS" || u.contains("ADVANCED SSA") || u.contains("S_TEXT/ASS") => "ASS",
        _ if u == "SSA" || u.contains("S_TEXT/SSA") => "SSA",
        _ if u.contains("MOV_TEXT") || u.contains("TIMED TEXT") || u == "TX3G" || u.contains("MP4TT") => "MOV_TEXT",
        _ if u.contains("TTML") => "TTML",
        _ if u.contains("608") => "EIA-608",
        _ if u.contains("708") => "EIA-708",
        _ if u.is_empty() => "Unknown",
        _ => return format.to_string(),
    }
    .to_string()
}

fn yes(v: &Value, key: &str) -> bool {
    matches!(s(v, key).to_ascii_lowercase().as_str(), "yes" | "true" | "1")
}

// ---------------------------------------------------------------- mediainfo

pub fn parse_mediainfo(v: &Value, path: &Path) -> MediaInfo {
    // Either {"media": {"track": [...]}} or an array of those (multiple files).
    let tracks: Vec<Value> = match v {
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.pointer("/media/track").and_then(Value::as_array).cloned())
            .max_by_key(|t| t.len())
            .unwrap_or_default(),
        _ => v.pointer("/media/track").and_then(Value::as_array).cloned().unwrap_or_default(),
    };
    let of_type = |t: &'static str| tracks.iter().filter(move |x| s(x, "@type").eq_ignore_ascii_case(t));

    let mut info = MediaInfo::default();
    if let Some(g) = of_type("General").next() {
        info.container = normalize_container(s(g, "Format"), path);
        info.duration = num(g, "Duration");
        info.overall_bitrate = num(g, "OverallBitRate") as u64;
    } else {
        info.container = container_from_extension(path);
    }

    if let Some(t) = of_type("Video").next() {
        let codec = normalize_video_codec(s(t, "Format"));
        let mut bit_depth = num(t, "BitDepth") as u32;
        if bit_depth == 0 {
            bit_depth = 8;
        }
        let hdr_format = s(t, "HDR_Format").to_string() + " " + s(t, "HDR_Format_Compatibility");
        let transfer = s(t, "transfer_characteristics").to_ascii_uppercase();
        let hdr_type = if hdr_format.contains("Dolby Vision") {
            Some("Dolby Vision".to_string())
        } else if hdr_format.contains("2094") || hdr_format.contains("HDR10+") {
            Some("HDR10+".into())
        } else if transfer.contains("PQ") || transfer.contains("2084") || hdr_format.contains("2086") {
            Some("HDR10".into())
        } else if transfer.contains("HLG") {
            Some("HLG".into())
        } else {
            None
        };
        let mut bitrate = num(t, "BitRate") as u64;
        if bitrate == 0 {
            bitrate = num(t, "BitRate_Nominal") as u64;
        }
        if info.duration <= 0.0 {
            info.duration = num(t, "Duration");
        }
        info.video = Some(VideoStream {
            codec,
            codec_tag: s(t, "CodecID").to_ascii_uppercase(),
            profile: s(t, "Format_Profile").to_string(),
            width: num(t, "Width") as u32,
            height: num(t, "Height") as u32,
            frame_rate: num(t, "FrameRate"),
            bit_depth,
            bitrate,
            hdr_type,
        });
    }

    for t in of_type("Audio") {
        let mut fmt = s(t, "Format").to_string();
        if fmt.eq_ignore_ascii_case("MLP FBA") || s(t, "Format_Commercial_IfAny").contains("TrueHD") {
            fmt = "TrueHD".into();
        }
        let mut channels = num(t, "Channels") as u32;
        if channels == 0 {
            channels = 2;
        }
        info.audio.push(AudioTrack {
            codec: normalize_audio_codec(&fmt),
            channels,
            sample_rate: num(t, "SamplingRate") as u32,
            bitrate: num(t, "BitRate") as u64,
            language: s(t, "Language").to_string(),
            title: s(t, "Title").to_string(),
            default: yes(t, "Default"),
        });
    }

    for t in of_type("Text") {
        let fmt = if s(t, "Format").is_empty() { s(t, "CodecID") } else { s(t, "Format") };
        info.subtitles.push(SubtitleTrack {
            format: normalize_subtitle_format(fmt),
            language: s(t, "Language").to_string(),
            embedded: true,
            forced: yes(t, "Forced"),
            path: None,
        });
    }
    info
}

// ---------------------------------------------------------------- ffprobe

fn tag<'a>(stream: &'a Value, key: &str) -> &'a str {
    let Some(tags) = stream.get("tags").and_then(Value::as_object) else { return "" };
    tags.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn stream_bitrate(st: &Value) -> u64 {
    let b = num(st, "bit_rate") as u64;
    if b > 0 {
        return b;
    }
    // Matroska stores statistics tags such as "BPS" / "BPS-eng".
    let t = tag(st, "BPS");
    if !t.is_empty() {
        return parse_num(t) as u64;
    }
    parse_num(tag(st, "BPS-eng")) as u64
}

pub fn parse_ffprobe(v: &Value, path: &Path) -> MediaInfo {
    let mut info = MediaInfo::default();
    let format = v.get("format").cloned().unwrap_or(Value::Null);
    info.container = normalize_container(s(&format, "format_name"), path);
    info.duration = num(&format, "duration");
    info.overall_bitrate = num(&format, "bit_rate") as u64;

    let empty = vec![];
    let streams = v.get("streams").and_then(Value::as_array).unwrap_or(&empty);
    for st in streams {
        let disposition = st.get("disposition").cloned().unwrap_or(Value::Null);
        let disp = |k: &str| disposition.get(k).and_then(Value::as_i64).unwrap_or(0) == 1;
        match s(st, "codec_type") {
            "video" if info.video.is_none() && !disp("attached_pic") => {
                let pix = s(st, "pix_fmt");
                let mut bit_depth = num(st, "bits_per_raw_sample") as u32;
                if bit_depth == 0 {
                    bit_depth = if pix.contains("12") {
                        12
                    } else if pix.contains("10") {
                        10
                    } else {
                        8
                    };
                }
                let transfer = s(st, "color_transfer");
                let has_dovi = st
                    .get("side_data_list")
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(|d| s(d, "side_data_type").contains("DOVI")));
                let hdr_type = if has_dovi {
                    Some("Dolby Vision".to_string())
                } else if transfer == "smpte2084" {
                    Some("HDR10".into())
                } else if transfer == "arib-std-b67" {
                    Some("HLG".into())
                } else {
                    None
                };
                let tag_str = s(st, "codec_tag_string");
                let codec_tag = if tag_str.starts_with('[') { String::new() } else { tag_str.to_ascii_uppercase() };
                let mut fr = parse_num(s(st, "avg_frame_rate"));
                if fr <= 0.0 {
                    fr = parse_num(s(st, "r_frame_rate"));
                }
                info.video = Some(VideoStream {
                    codec: normalize_video_codec(match s(st, "codec_name") {
                        "mpeg2video" => "MPEG-2",
                        "mpeg4" => "MPEG4",
                        "vc1" | "wmv3" => "VC-1",
                        other => other,
                    }),
                    codec_tag,
                    profile: s(st, "profile").to_string(),
                    width: num(st, "width") as u32,
                    height: num(st, "height") as u32,
                    frame_rate: (fr * 1000.0).round() / 1000.0,
                    bit_depth,
                    bitrate: stream_bitrate(st),
                    hdr_type,
                });
            }
            "audio" => {
                let codec_name = s(st, "codec_name");
                let profile = s(st, "profile");
                let codec = if codec_name == "dts" && profile.contains("MA") {
                    "DTS".to_string()
                } else {
                    normalize_audio_codec(codec_name)
                };
                info.audio.push(AudioTrack {
                    codec,
                    channels: num(st, "channels") as u32,
                    sample_rate: num(st, "sample_rate") as u32,
                    bitrate: stream_bitrate(st),
                    language: tag(st, "language").to_string(),
                    title: tag(st, "title").to_string(),
                    default: disp("default"),
                });
            }
            "subtitle" => info.subtitles.push(SubtitleTrack {
                format: normalize_subtitle_format(s(st, "codec_name")),
                language: tag(st, "language").to_string(),
                embedded: true,
                forced: disp("forced"),
                path: None,
            }),
            _ => {}
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_mediainfo_json() {
        let v = json!({"media": {"track": [
            {"@type": "General", "Format": "Matroska", "Duration": "5400.123", "OverallBitRate": "8000000"},
            {"@type": "Video", "Format": "HEVC", "CodecID": "V_MPEGH/ISO/HEVC", "Format_Profile": "Main 10",
             "Width": "1920", "Height": "1080", "FrameRate": "23.976", "BitDepth": "10", "BitRate": "7000000",
             "HDR_Format": "SMPTE ST 2086", "transfer_characteristics": "PQ"},
            {"@type": "Audio", "Format": "E-AC-3", "Channels": "6", "SamplingRate": "48000", "BitRate": "640000", "Language": "en", "Default": "Yes"},
            {"@type": "Audio", "Format": "MLP FBA", "Format_Commercial_IfAny": "Dolby TrueHD with Dolby Atmos", "Channels": "8"},
            {"@type": "Text", "Format": "PGS", "Language": "de", "Forced": "Yes"},
            {"@type": "Text", "Format": "UTF-8", "Language": "en"}
        ]}});
        let i = parse_mediainfo(&v, Path::new("/x/a.mkv"));
        assert_eq!(i.container, "MKV");
        assert_eq!(i.duration, 5400.123);
        let vs = i.video.unwrap();
        assert_eq!(vs.codec, "H.265");
        assert_eq!(vs.bit_depth, 10);
        assert_eq!(vs.hdr_type.as_deref(), Some("HDR10"));
        assert_eq!(i.audio[0].codec, "EAC3");
        assert!(i.audio[0].default);
        assert_eq!(i.audio[1].codec, "TrueHD");
        assert_eq!(i.subtitles[0].format, "PGS");
        assert!(i.subtitles[0].forced);
        assert_eq!(i.subtitles[1].format, "SRT");
    }

    #[test]
    fn bt2020_alone_is_not_hdr() {
        let v = json!({"media": {"track": [
            {"@type": "Video", "Format": "HEVC", "colour_primaries": "BT.2020", "transfer_characteristics": "BT.709"}
        ]}});
        assert_eq!(parse_mediainfo(&v, Path::new("a.mp4")).video.unwrap().hdr_type, None);
    }

    #[test]
    fn parses_ffprobe_json() {
        let v = json!({
            "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "60.0", "bit_rate": "5000000"},
            "streams": [
                {"codec_type": "video", "codec_name": "h264", "codec_tag_string": "avc1", "profile": "High",
                 "width": 1280, "height": 720, "avg_frame_rate": "24000/1001", "pix_fmt": "yuv420p", "bit_rate": "4800000",
                 "disposition": {"default": 1, "attached_pic": 0}},
                {"codec_type": "audio", "codec_name": "aac", "channels": 2, "sample_rate": "48000", "bit_rate": "192000",
                 "tags": {"language": "eng"}, "disposition": {"default": 1}},
                {"codec_type": "subtitle", "codec_name": "mov_text", "tags": {"language": "eng"}}
            ]});
        let i = parse_ffprobe(&v, Path::new("a.mp4"));
        assert_eq!(i.container, "MP4");
        let vs = i.video.unwrap();
        assert_eq!(vs.codec, "H.264");
        assert_eq!(vs.codec_tag, "AVC1");
        assert_eq!(vs.frame_rate, 23.976);
        assert_eq!(i.audio[0].codec, "AAC");
        assert_eq!(i.subtitles[0].format, "MOV_TEXT");
    }

    #[test]
    fn numbers_with_units() {
        assert_eq!(parse_num("1 920 pixels"), 1920.0);
        assert_eq!(parse_num("24000/1001"), 24000.0 / 1001.0);
        assert_eq!(parse_num(""), 0.0);
    }
}
