//! Scores a probed file on two independent axes:
//! * **compatibility**: how likely it is to direct-play on the configured clients;
//! * **quality**: how good the encode is (resolution, bitrate, HDR, audio).

use super::model::*;
use crate::settings::{CompatibilityProfile, QualityProfile};

pub fn assess(info: &MediaInfo, file_size: u64, path: &str, compat: &CompatibilityProfile, quality: &QualityProfile) -> Assessment {
    let mut issues = Vec::new();
    let compat_score = compatibility(info, file_size, compat, &mut issues);
    let quality_score = quality_score(info, quality, &mut issues);

    let compat_verdict = if info.video.is_none() && info.container.is_empty() {
        "unknown"
    } else if compat_score >= compat.thresholds.optimal {
        "optimal"
    } else if compat_score >= compat.thresholds.good {
        "good"
    } else {
        "poor"
    }
    .to_string();

    let t = &quality.thresholds;
    let quality_verdict = if info.video.is_none() {
        "unknown"
    } else if quality_score >= t.excellent {
        "excellent"
    } else if quality_score >= t.good {
        "good"
    } else if quality_score >= t.fair {
        "fair"
    } else {
        "poor"
    }
    .to_string();

    issues.sort_by(|a, b| b.severity.cmp(&a.severity).then(b.penalty.cmp(&a.penalty)));
    let ffmpeg_command = super::fixes::ffmpeg_command_for(info, compat, &issues, path);
    let summary = summarize(info, compat_score, &compat_verdict, quality_score, &quality_verdict, &issues);

    Assessment { compat_score, compat_verdict, quality_score, quality_verdict, issues, ffmpeg_command, summary }
}

fn lookup(map: &std::collections::BTreeMap<String, bool>, key: &str) -> bool {
    if let Some(v) = map.get(key) {
        return *v;
    }
    map.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| *v).unwrap_or(false)
}

pub fn video_codec_supported(p: &CompatibilityProfile, codec: &str, bit_depth: u32) -> bool {
    let with_depth = format!("{codec} {bit_depth}-bit");
    if p.video_codecs.keys().any(|k| k.eq_ignore_ascii_case(&with_depth)) {
        return lookup(&p.video_codecs, &with_depth);
    }
    lookup(&p.video_codecs, codec)
}

pub fn audio_codec_supported(p: &CompatibilityProfile, codec: &str) -> bool {
    lookup(&p.audio_codecs, codec)
}

pub fn subtitle_supported(p: &CompatibilityProfile, format: &str) -> bool {
    format.is_empty() || lookup(&p.subtitle_formats, format)
}

pub fn container_supported(p: &CompatibilityProfile, container: &str) -> bool {
    lookup(&p.containers, container)
}

fn issue(issues: &mut Vec<Issue>, code: &str, area: IssueArea, severity: Severity, penalty: i32, message: String, fix: Option<&str>) {
    issues.push(Issue { code: code.into(), area, severity, message, penalty, fix: fix.map(Into::into) });
}

fn compatibility(info: &MediaInfo, file_size: u64, p: &CompatibilityProfile, issues: &mut Vec<Issue>) -> i32 {
    use IssueArea::Compatibility as C;
    let w = &p.weights;
    let mut score = 100;

    match &info.video {
        Some(v) => {
            if !video_codec_supported(p, &v.codec, v.bit_depth) {
                score -= w.unsupported_video_codec;
                issue(
                    issues,
                    "video_codec",
                    C,
                    Severity::Critical,
                    w.unsupported_video_codec,
                    format!("{} {}-bit video is not direct-playable on your clients and will be transcoded", v.codec, v.bit_depth),
                    Some("Re-encode the video to H.264 8-bit (or another supported codec)"),
                );
            }
            if !lookup(&p.bit_depths, &v.bit_depth.to_string()) {
                score -= w.unsupported_bit_depth;
                issue(
                    issues,
                    "bit_depth",
                    C,
                    Severity::Warning,
                    w.unsupported_bit_depth,
                    format!("{}-bit colour depth is not supported by every client", v.bit_depth),
                    Some("Re-encode to 8-bit (yuv420p)"),
                );
            }
            if !v.codec_tag.is_empty() && is_mp4_family(&info.container) && !codec_tag_ok(&v.codec, &v.codec_tag) {
                score -= w.incorrect_codec_tag;
                issue(
                    issues,
                    "codec_tag",
                    C,
                    Severity::Warning,
                    w.incorrect_codec_tag,
                    format!("Codec tag {} is wrong for {} (Apple devices refuse to play it)", v.codec_tag, v.codec),
                    Some(if v.codec == "H.265" { "Remux with -tag:v hvc1" } else { "Remux with the standard codec tag" }),
                );
            }
        }
        None => {
            score -= w.unsupported_video_codec;
            issue(issues, "no_video", C, Severity::Critical, w.unsupported_video_codec, "No video stream could be read".into(), None);
        }
    }

    let mut bad_audio: Vec<&str> = info.audio.iter().filter(|a| !audio_codec_supported(p, &a.codec)).map(|a| a.codec.as_str()).collect();
    bad_audio.dedup();
    if !bad_audio.is_empty() {
        let any_good = info.audio.iter().any(|a| audio_codec_supported(p, &a.codec));
        // Only the tracks a client picks get transcoded; if a compatible alternative exists the
        // impact is smaller.
        let penalty = if any_good { w.unsupported_audio_codec / 2 } else { w.unsupported_audio_codec };
        score -= penalty;
        issue(
            issues,
            "audio_codec",
            C,
            if any_good { Severity::Warning } else { Severity::Critical },
            penalty,
            if any_good {
                format!("{} audio is not supported (a compatible track also exists)", bad_audio.join(", "))
            } else {
                format!("{} audio is not supported and will be transcoded", bad_audio.join(", "))
            },
            Some("Convert the audio to AAC, or add an AAC track"),
        );
    }

    if !container_supported(p, &info.container) {
        score -= w.unsupported_container;
        issue(
            issues,
            "container",
            C,
            Severity::Critical,
            w.unsupported_container,
            format!(
                "{} container is not supported and will be remuxed on the fly",
                if info.container.is_empty() { "Unknown" } else { &info.container }
            ),
            Some("Remux to MP4 (lossless, fast)"),
        );
    }

    let mut bad_subs: Vec<&str> = info.subtitles.iter().filter(|s| !subtitle_supported(p, &s.format)).map(|s| s.format.as_str()).collect();
    bad_subs.sort();
    bad_subs.dedup();
    if !bad_subs.is_empty() {
        score -= w.unsupported_subtitle_format;
        issue(
            issues,
            "subtitle_format",
            C,
            Severity::Warning,
            w.unsupported_subtitle_format,
            format!("{} subtitles will be burned in, forcing a transcode when enabled", bad_subs.join(", ")),
            Some("Convert text subtitles to SRT; image subtitles need OCR or removal"),
        );
    }

    if is_mp4_family(&info.container) && info.fast_start == Some(false) {
        score -= w.missing_fast_start;
        issue(
            issues,
            "fast_start",
            C,
            Severity::Info,
            w.missing_fast_start,
            "MP4 index (moov atom) is at the end of the file, so streaming starts slower".into(),
            Some("Remux with -movflags +faststart"),
        );
    }

    if let Some(mbps) = bitrate_mbps(info, file_size) {
        if mbps > w.high_bitrate_threshold_mbps {
            score -= w.high_bitrate;
            issue(
                issues,
                "high_bitrate",
                C,
                Severity::Info,
                w.high_bitrate,
                format!("{mbps:.1} Mbps may buffer or be transcoded on slow or remote connections"),
                Some("Lower the bitrate, or raise the client's streaming limit"),
            );
        }
    }

    score.clamp(0, 100)
}

pub fn is_mp4_family(container: &str) -> bool {
    matches!(container.to_ascii_uppercase().as_str(), "MP4" | "M4V" | "MOV")
}

pub fn codec_tag_ok(codec: &str, tag: &str) -> bool {
    let tag = tag.to_ascii_uppercase();
    match codec {
        "H.265" => matches!(tag.as_str(), "HVC1" | "DVH1"),
        "H.264" => matches!(tag.as_str(), "AVC1" | "AVC3" | "H264"),
        "VP9" => tag == "VP09",
        "AV1" => tag == "AV01",
        _ => true,
    }
}

fn bitrate_mbps(info: &MediaInfo, file_size: u64) -> Option<f64> {
    if info.overall_bitrate > 0 {
        return Some(info.overall_bitrate as f64 / 1_000_000.0);
    }
    if file_size > 0 && info.duration > 0.0 {
        let mbps = file_size as f64 * 8.0 / info.duration / 1_000_000.0;
        // Anything above 1 Gbps means the duration is wrong, not the file.
        return (mbps < 1000.0).then_some(mbps);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolutionTier {
    Sd,
    Hd720,
    Hd1080,
    Uhd4k,
}

impl ResolutionTier {
    pub fn of(width: u32, height: u32) -> Self {
        if width >= 3200 || height >= 2000 {
            Self::Uhd4k
        } else if width >= 1800 || height >= 1000 {
            Self::Hd1080
        } else if width >= 1200 || height >= 700 {
            Self::Hd720
        } else {
            Self::Sd
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sd => "SD",
            Self::Hd720 => "720p",
            Self::Hd1080 => "1080p",
            Self::Uhd4k => "4K",
        }
    }
}

fn is_modern_codec(codec: &str) -> bool {
    matches!(codec, "H.265" | "VP9" | "AV1")
}

fn is_legacy_codec(codec: &str) -> bool {
    matches!(codec, "MPEG-2" | "MPEG-4 Visual" | "VC-1" | "MPEG-1" | "H.263" | "WMV" | "RealVideo" | "Theora")
}

/// Best estimate of the video stream bitrate in bits/s.
pub fn video_bitrate(info: &MediaInfo) -> u64 {
    match &info.video {
        Some(v) if v.bitrate > 0 => v.bitrate,
        Some(_) if info.overall_bitrate > 0 => {
            let audio: u64 = info.audio.iter().map(|a| a.bitrate).sum();
            info.overall_bitrate.saturating_sub(audio)
        }
        _ => 0,
    }
}

fn quality_score(info: &MediaInfo, p: &QualityProfile, issues: &mut Vec<Issue>) -> i32 {
    use IssueArea::Quality as Q;
    let Some(v) = &info.video else { return 0 };
    let mut score = 100;
    let tier = ResolutionTier::of(v.width, v.height);

    let (penalty, sev) = match tier {
        ResolutionTier::Uhd4k => (0, Severity::Info),
        ResolutionTier::Hd1080 => (p.penalty_1080p, Severity::Info),
        ResolutionTier::Hd720 => (p.penalty_720p, Severity::Warning),
        ResolutionTier::Sd => (p.penalty_sd, Severity::Critical),
    };
    if penalty > 0 && v.width > 0 {
        score -= penalty;
        issue(
            issues,
            "resolution",
            Q,
            sev,
            penalty,
            format!("{} ({}x{})", tier.label(), v.width, v.height)
                + match tier {
                    ResolutionTier::Sd => " is standard definition; an HD release would look much better",
                    ResolutionTier::Hd720 => " is below Full HD",
                    _ => " is Full HD; a 4K release exists for many titles",
                },
            (tier < ResolutionTier::Hd1080).then_some("Look for a 1080p or better release"),
        );
    }

    let vb = video_bitrate(info);
    if vb > 0 && v.width > 0 && v.height > 0 {
        let fps = if v.frame_rate > 0.0 { v.frame_rate } else { 24.0 };
        let bpp = vb as f64 / (v.width as f64 * v.height as f64 * fps);
        let mut min = p.min_bits_per_pixel;
        if is_modern_codec(&v.codec) {
            min *= 0.6;
        } else if is_legacy_codec(&v.codec) {
            min *= 1.5;
        }
        let mbps = vb as f64 / 1e6;
        if bpp < min / 2.0 {
            score -= p.penalty_very_low_bitrate;
            issue(
                issues,
                "very_low_bitrate",
                Q,
                Severity::Critical,
                p.penalty_very_low_bitrate,
                format!("Video bitrate is very low for {} {} ({mbps:.1} Mbps): expect blocking and banding", tier.label(), v.codec),
                Some("Replace with a higher bitrate release"),
            );
        } else if bpp < min {
            score -= p.penalty_low_bitrate;
            issue(
                issues,
                "low_bitrate",
                Q,
                Severity::Warning,
                p.penalty_low_bitrate,
                format!("Video bitrate is on the low side for {} {} ({mbps:.1} Mbps)", tier.label(), v.codec),
                Some("Consider a higher bitrate release"),
            );
        }
    }

    if is_legacy_codec(&v.codec) {
        score -= p.penalty_legacy_codec;
        issue(
            issues,
            "legacy_codec",
            Q,
            Severity::Warning,
            p.penalty_legacy_codec,
            format!("{} is an outdated codec with poor efficiency", v.codec),
            Some("Replace with an H.264/H.265 release"),
        );
    }

    if tier == ResolutionTier::Uhd4k && v.hdr_type.is_none() {
        score -= p.penalty_sdr_4k;
        issue(
            issues,
            "sdr_4k",
            Q,
            Severity::Info,
            p.penalty_sdr_4k,
            "4K without HDR misses most of the benefit of a UHD release".into(),
            Some("Look for an HDR10 / Dolby Vision release"),
        );
    }

    if !info.audio.is_empty() {
        let max_ch = info.audio.iter().map(|a| a.channels).max().unwrap_or(0);
        if max_ch <= 2 {
            score -= p.penalty_stereo_only;
            issue(
                issues,
                "stereo_only",
                Q,
                Severity::Info,
                p.penalty_stereo_only,
                format!("Only {} audio is available, no surround track", if max_ch == 1 { "mono" } else { "stereo" }),
                None,
            );
        }
        let best = info.audio.iter().max_by_key(|a| (a.channels, a.bitrate)).unwrap();
        let lossless = matches!(best.codec.as_str(), "FLAC" | "TrueHD" | "PCM" | "ALAC") || best.codec.contains("DTS-HD");
        if !lossless && best.bitrate > 0 && best.bitrate < 96_000 * u64::from(best.channels.max(2)) / 2 {
            score -= p.penalty_low_audio_bitrate;
            issue(
                issues,
                "low_audio_bitrate",
                Q,
                Severity::Info,
                p.penalty_low_audio_bitrate,
                format!("Audio bitrate is low ({} kbps for {} channels)", best.bitrate / 1000, best.channels),
                None,
            );
        }
    } else {
        score -= 20;
        issue(issues, "no_audio", Q, Severity::Warning, 20, "File has no audio track".into(), None);
    }

    language_checks(info, p, &mut score, issues);

    score.clamp(0, 100)
}

fn language_list(langs: &[String]) -> String {
    langs.iter().map(|l| super::files::language_name(l).unwrap_or_else(|| l.clone())).collect::<Vec<_>>().join(" or ")
}

/// Missing audio or subtitle languages, and subtitles that only exist as images.
fn language_checks(info: &MediaInfo, p: &QualityProfile, score: &mut i32, issues: &mut Vec<Issue>) {
    use super::files::{same_language, unknown_language};
    use IssueArea::Quality as Q;
    let wanted = |tag: &str, langs: &[String]| langs.iter().any(|l| same_language(tag, l));

    let audio_langs = &p.preferred_audio_languages;
    // Untagged tracks might be in the right language, so only flag files whose tracks are all tagged.
    if !audio_langs.is_empty()
        && !info.audio.is_empty()
        && !info.audio.iter().any(|a| unknown_language(&a.language))
        && !info.audio.iter().any(|a| wanted(&a.language, audio_langs))
    {
        *score -= p.penalty_missing_audio_language;
        let have: Vec<String> = info.audio.iter().map(|a| super::files::language_name(&a.language).unwrap_or_else(|| a.language.clone())).collect();
        issue(
            issues,
            "missing_audio_language",
            Q,
            Severity::Warning,
            p.penalty_missing_audio_language,
            format!("No {} audio (has {})", language_list(audio_langs), have.join(", ")),
            Some("Look for a release with the audio language you want"),
        );
    }

    let sub_langs = &p.preferred_subtitle_languages;
    if !sub_langs.is_empty() {
        let matching: Vec<&SubtitleTrack> = info.subtitles.iter().filter(|s| wanted(&s.language, sub_langs)).collect();
        if matching.is_empty() {
            *score -= p.penalty_missing_subtitle_language;
            issue(
                issues,
                "missing_subtitle_language",
                Q,
                Severity::Info,
                p.penalty_missing_subtitle_language,
                format!("No {} subtitles", language_list(sub_langs)),
                Some("Download subtitles with Bazarr, or place an .srt file next to the video"),
            );
        } else if matching.iter().all(|s| s.is_image_based()) {
            *score -= p.penalty_image_subtitles;
            issue(
                issues,
                "image_subtitles",
                Q,
                Severity::Warning,
                p.penalty_image_subtitles,
                format!(
                    "{} subtitles are only available as images ({}), which forces a transcode to burn them in",
                    language_list(sub_langs),
                    matching.iter().map(|s| s.format.as_str()).collect::<Vec<_>>().join(", ")
                ),
                Some("Add a text (.srt) subtitle, e.g. with Bazarr or OCR in Subtitle Edit"),
            );
        }
    }
}

fn describe(info: &MediaInfo) -> String {
    let Some(v) = &info.video else { return "unreadable file".into() };
    let mut parts = vec![ResolutionTier::of(v.width, v.height).label().to_string(), v.codec.clone()];
    if let Some(h) = &v.hdr_type {
        parts.push(h.clone());
    }
    if let Some(a) = info.audio.iter().max_by_key(|a| a.channels) {
        parts.push(format!("{} {}", a.codec, channel_label(a.channels)));
    }
    if !info.container.is_empty() {
        parts.push(info.container.clone());
    }
    parts.join(", ")
}

pub fn channel_label(ch: u32) -> String {
    match ch {
        0 => "?".into(),
        1 => "mono".into(),
        2 => "stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

fn summarize(info: &MediaInfo, cs: i32, cv: &str, qs: i32, qv: &str, issues: &[Issue]) -> String {
    let compat_blockers: Vec<&str> =
        issues.iter().filter(|i| i.area == IssueArea::Compatibility && i.severity >= Severity::Warning).map(|i| i.code.as_str()).collect();
    let mut s = format!("{}. Compatibility {cv} ({cs}/100)", describe(info));
    if !compat_blockers.is_empty() {
        s += &format!(": problems with {}", compat_blockers.join(", ").replace('_', " "));
    }
    s += &format!(". Quality {qv} ({qs}/100)");
    let q: Vec<&str> = issues.iter().filter(|i| i.area == IssueArea::Quality && i.severity >= Severity::Warning).map(|i| i.code.as_str()).collect();
    if !q.is_empty() {
        s += &format!(": {}", q.join(", ").replace('_', " "));
    }
    s.push('.');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hevc_mkv() -> MediaInfo {
        MediaInfo {
            container: "MKV".into(),
            duration: 3600.0,
            overall_bitrate: 8_000_000,
            fast_start: None,
            video: Some(VideoStream {
                codec: "H.265".into(),
                codec_tag: "".into(),
                profile: "Main 10".into(),
                width: 1920,
                height: 1080,
                frame_rate: 23.976,
                bit_depth: 10,
                bitrate: 7_000_000,
                hdr_type: Some("HDR10".into()),
            }),
            audio: vec![AudioTrack {
                codec: "DTS".into(),
                channels: 6,
                sample_rate: 48000,
                bitrate: 1_509_000,
                language: "en".into(),
                ..Default::default()
            }],
            subtitles: vec![SubtitleTrack { format: "PGS".into(), language: "en".into(), embedded: true, ..Default::default() }],
        }
    }

    fn h264_mp4() -> MediaInfo {
        MediaInfo {
            container: "MP4".into(),
            duration: 3600.0,
            overall_bitrate: 6_000_000,
            fast_start: Some(true),
            video: Some(VideoStream {
                codec: "H.264".into(),
                codec_tag: "AVC1".into(),
                profile: "High".into(),
                width: 1920,
                height: 1080,
                frame_rate: 24.0,
                bit_depth: 8,
                bitrate: 5_600_000,
                hdr_type: None,
            }),
            audio: vec![AudioTrack {
                codec: "AAC".into(),
                channels: 6,
                sample_rate: 48000,
                bitrate: 384_000,
                language: "en".into(),
                ..Default::default()
            }],
            subtitles: vec![],
        }
    }

    #[test]
    fn optimal_file_scores_high() {
        let a = assess(&h264_mp4(), 0, "/m/a.mp4", &Default::default(), &Default::default());
        assert_eq!(a.compat_score, 100);
        assert_eq!(a.compat_verdict, "optimal");
        assert!(a.quality_score >= 85, "{a:?}");
        assert!(a.ffmpeg_command.is_none());
    }

    #[test]
    fn missing_fast_start_penalised() {
        let mut m = h264_mp4();
        m.fast_start = Some(false);
        let a = assess(&m, 0, "/m/a.mp4", &Default::default(), &Default::default());
        assert_eq!(a.compat_score, 95);
        assert!(a.ffmpeg_command.unwrap().contains("+faststart"));
    }

    #[test]
    fn problematic_file_scores_low_with_fix() {
        let a = assess(&hevc_mkv(), 0, "/m/a.mkv", &Default::default(), &Default::default());
        // 10-bit HEVC (35) + 10-bit depth (18) + DTS (25) + MKV (30) + PGS (8)
        assert_eq!(a.compat_score, 0);
        assert_eq!(a.compat_verdict, "poor");
        let cmd = a.ffmpeg_command.unwrap();
        assert!(cmd.contains("libx264"));
        assert!(cmd.contains("aac"));
        assert!(cmd.ends_with(".optimized.mp4'") || cmd.contains(".optimized.mp4"));
        assert!(a.issues[0].severity == Severity::Critical);
    }

    #[test]
    fn hevc_8bit_supported() {
        let p = CompatibilityProfile::default();
        assert!(video_codec_supported(&p, "H.265", 8));
        assert!(!video_codec_supported(&p, "H.265", 10));
        assert!(!video_codec_supported(&p, "Unknown", 8));
    }

    #[test]
    fn low_bitrate_detected() {
        let mut m = h264_mp4();
        m.video.as_mut().unwrap().bitrate = 1_000_000;
        let a = assess(&m, 0, "/m/a.mp4", &Default::default(), &Default::default());
        assert!(a.issues.iter().any(|i| i.code == "very_low_bitrate"));
    }

    #[test]
    fn language_checks() {
        let q =
            QualityProfile { preferred_audio_languages: vec!["de".into()], preferred_subtitle_languages: vec!["eng".into()], ..Default::default() };
        // English-only audio, English PGS subtitles only.
        let a = assess(&hevc_mkv(), 0, "/m/a.mkv", &Default::default(), &q);
        assert!(a.issues.iter().any(|i| i.code == "missing_audio_language"), "{:?}", a.issues);
        assert!(a.issues.iter().any(|i| i.code == "image_subtitles"));
        // No subtitles at all.
        let a = assess(&h264_mp4(), 0, "/m/a.mp4", &Default::default(), &q);
        assert!(a.issues.iter().any(|i| i.code == "missing_subtitle_language"));
        // Untagged audio is not flagged; checks are off by default.
        let mut m = h264_mp4();
        m.audio[0].language = String::new();
        let a = assess(&m, 0, "/m/a.mp4", &Default::default(), &q);
        assert!(!a.issues.iter().any(|i| i.code == "missing_audio_language"));
        let a = assess(&hevc_mkv(), 0, "/m/a.mkv", &Default::default(), &Default::default());
        assert!(!a.issues.iter().any(|i| i.code.starts_with("missing_") || i.code == "image_subtitles"));
    }

    #[test]
    fn resolution_tiers() {
        assert_eq!(ResolutionTier::of(1920, 800), ResolutionTier::Hd1080);
        assert_eq!(ResolutionTier::of(3840, 1600), ResolutionTier::Uhd4k);
        assert_eq!(ResolutionTier::of(1280, 534), ResolutionTier::Hd720);
        assert_eq!(ResolutionTier::of(720, 480), ResolutionTier::Sd);
    }
}
