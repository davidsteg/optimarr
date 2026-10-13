//! Builds a single ffmpeg command line that fixes every compatibility issue that can be fixed
//! by remuxing or re-encoding.

use super::model::{Issue, IssueArea, MediaInfo};
use super::rating::{audio_codec_supported, container_supported, subtitle_supported, video_codec_supported};
use crate::settings::CompatibilityProfile;

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Text subtitle formats that ffmpeg can convert between.
fn is_text_sub(format: &str) -> bool {
    matches!(format, "SRT" | "VTT" | "ASS" | "SSA" | "MOV_TEXT" | "TTML")
}

/// Returns `None` when nothing needs to change.
pub fn ffmpeg_command_for(info: &MediaInfo, p: &CompatibilityProfile, issues: &[Issue], input: &str) -> Option<String> {
    let fixable = issues.iter().any(|i| {
        i.area == IssueArea::Compatibility
            && matches!(
                i.code.as_str(),
                "video_codec" | "bit_depth" | "codec_tag" | "audio_codec" | "container" | "subtitle_format" | "fast_start"
            )
    });
    let video = info.video.as_ref()?;
    if !fixable {
        return None;
    }

    let to_mp4 = container_supported(p, "MP4");
    let ext = if to_mp4 { "mp4" } else if container_supported(p, "MKV") { "mkv" } else { "mp4" };
    let stem = input.rsplit_once('.').map(|(s, _)| s).unwrap_or(input);
    let output = format!("{stem}.optimized.{ext}");

    let mut args: Vec<String> = vec!["ffmpeg".into(), "-hide_banner".into(), "-i".into(), shell_quote(input)];
    args.extend(["-map".into(), "0:v:0".into(), "-map".into(), "0:a?".into()]);

    // Video
    let reencode_video = !video_codec_supported(p, &video.codec, video.bit_depth)
        || !p.bit_depths.get(&video.bit_depth.to_string()).copied().unwrap_or(false);
    if reencode_video {
        args.extend(
            ["-c:v", "libx264", "-preset", "slow", "-crf", "19", "-pix_fmt", "yuv420p", "-profile:v", "high"]
                .map(String::from),
        );
        if video.hdr_type.is_some() {
            // Tone-map HDR to SDR so colours are not washed out.
            args.extend([
                "-vf".into(),
                "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p".into(),
            ]);
        }
    } else {
        args.extend(["-c:v".into(), "copy".into()]);
        if ext == "mp4" {
            match video.codec.as_str() {
                "H.265" => args.extend(["-tag:v".into(), "hvc1".into()]),
                "H.264" => args.extend(["-tag:v".into(), "avc1".into()]),
                _ => {}
            }
        }
    }

    // Audio, per track
    for (i, a) in info.audio.iter().enumerate() {
        if audio_codec_supported(p, &a.codec) {
            args.extend([format!("-c:a:{i}"), "copy".into()]);
        } else {
            let ch = a.channels.clamp(1, 6);
            let kbps = match ch {
                1 => 128,
                2 => 192,
                _ => 384,
            };
            args.extend([
                format!("-c:a:{i}"),
                "aac".into(),
                format!("-ac:a:{i}"),
                ch.to_string(),
                format!("-b:a:{i}"),
                format!("{kbps}k"),
            ]);
        }
    }

    // Subtitles: keep embedded text subs (converted when needed), drop image subs that the
    // target container cannot carry or the clients cannot render.
    let embedded: Vec<_> = info.subtitles.iter().filter(|s| s.embedded).collect();
    let mut out_index = 0;
    for (i, s) in embedded.iter().enumerate() {
        let keep_as_is = subtitle_supported(p, &s.format) && (ext != "mp4" || s.format == "MOV_TEXT");
        if keep_as_is {
            args.extend(["-map".into(), format!("0:s:{i}"), format!("-c:s:{out_index}"), "copy".into()]);
            out_index += 1;
        } else if is_text_sub(&s.format) {
            let target = if ext == "mp4" { "mov_text" } else { "srt" };
            args.extend(["-map".into(), format!("0:s:{i}"), format!("-c:s:{out_index}"), target.into()]);
            out_index += 1;
        }
        // Image-based subtitles are dropped (comment appended below).
    }

    if ext == "mp4" {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args.push(shell_quote(&output));

    let mut cmd = args.join(" ");
    if embedded.iter().any(|s| s.is_image_based()) {
        cmd.push_str("  # image-based subtitles dropped; extract with OCR (e.g. Subtitle Edit) if needed");
    }
    Some(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::model::*;

    #[test]
    fn quotes_paths() {
        assert_eq!(shell_quote("it's.mkv"), r"'it'\''s.mkv'");
    }

    #[test]
    fn remux_only_when_codecs_fine() {
        let info = MediaInfo {
            container: "MKV".into(),
            video: Some(VideoStream { codec: "H.264".into(), bit_depth: 8, width: 1920, height: 1080, ..Default::default() }),
            audio: vec![AudioTrack { codec: "AAC".into(), channels: 2, ..Default::default() }],
            subtitles: vec![SubtitleTrack { format: "SRT".into(), embedded: true, ..Default::default() }],
            ..Default::default()
        };
        let p = CompatibilityProfile::default();
        let issues = vec![Issue {
            code: "container".into(),
            area: IssueArea::Compatibility,
            severity: Severity::Critical,
            message: String::new(),
            penalty: 30,
            fix: None,
        }];
        let cmd = ffmpeg_command_for(&info, &p, &issues, "/m/Movie.mkv").unwrap();
        assert!(cmd.contains("-c:v copy -tag:v avc1"));
        assert!(cmd.contains("-c:a:0 copy"));
        assert!(cmd.contains("-c:s:0 mov_text"));
        assert!(cmd.ends_with("'/m/Movie.optimized.mp4'"));
    }
}
