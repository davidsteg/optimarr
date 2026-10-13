//! Rough estimate of how much space re-encoding the video stream would save.
//!
//! The target bitrate comes from a bits-per-pixel budget that gives visually good results for
//! each codec (x265 / SVT-AV1 at typical "transparent" settings), scaled down slightly for 4K
//! where encoders are more efficient per pixel. Audio and subtitles are kept as they are.

use serde::Serialize;

use super::model::MediaInfo;
use super::rating::video_bitrate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Hevc,
    Av1,
}

impl Target {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "hevc" | "h265" | "h.265" | "x265" => Some(Self::Hevc),
            "av1" => Some(Self::Av1),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hevc => "H.265",
            Self::Av1 => "AV1",
        }
    }

    fn bits_per_pixel(self) -> f64 {
        match self {
            Self::Hevc => 0.07,
            Self::Av1 => 0.05,
        }
    }

    /// Codecs that are already as efficient as the target; re-encoding them saves little.
    fn already_efficient(self, codec: &str) -> bool {
        match self {
            Self::Hevc => matches!(codec, "H.265" | "AV1" | "VP9"),
            Self::Av1 => codec == "AV1",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub target: Target,
    pub current_size: u64,
    pub estimated_size: u64,
    pub savings: u64,
    /// Video bitrate the estimate assumes, in bits/s.
    pub target_video_bitrate: u64,
}

/// Minimum saving (fraction of the file) worth reporting.
const MIN_FRACTION: f64 = 0.15;

pub fn estimate(info: &MediaInfo, file_size: u64, target: Target) -> Option<Estimate> {
    let v = info.video.as_ref()?;
    if file_size == 0 || info.duration <= 0.0 || v.width == 0 || v.height == 0 || target.already_efficient(&v.codec) {
        return None;
    }
    let current_vb = video_bitrate(info);
    if current_vb == 0 {
        return None;
    }
    let fps = if v.frame_rate > 0.0 { v.frame_rate } else { 24.0 };
    let pixels = v.width as f64 * v.height as f64;
    let mut bpp = target.bits_per_pixel();
    if pixels >= 3200.0 * 1800.0 {
        bpp *= 0.75;
    }
    if v.hdr_type.is_some() || v.bit_depth > 8 {
        bpp *= 1.15;
    }
    let target_vb = ((pixels * fps * bpp) as u64).min(current_vb);
    let saved_bits = (current_vb - target_vb) as f64 * info.duration;
    let savings = ((saved_bits / 8.0) as u64).min(file_size);
    if (savings as f64) < file_size as f64 * MIN_FRACTION {
        return None;
    }
    Some(Estimate { target, current_size: file_size, estimated_size: file_size - savings, savings, target_video_bitrate: target_vb })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::model::VideoStream;

    fn h264(bitrate: u64) -> MediaInfo {
        MediaInfo {
            container: "MKV".into(),
            duration: 7200.0,
            video: Some(VideoStream {
                codec: "H.264".into(),
                width: 1920,
                height: 1080,
                frame_rate: 24.0,
                bit_depth: 8,
                bitrate,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn bloated_h264_saves_space() {
        let info = h264(15_000_000);
        let size = 15_000_000 / 8 * 7200;
        let e = estimate(&info, size, Target::Hevc).unwrap();
        // 1920*1080*24*0.07 ≈ 3.5 Mbps target
        assert!(e.target_video_bitrate > 3_000_000 && e.target_video_bitrate < 4_000_000, "{e:?}");
        assert!(e.savings > size / 2);
        assert!(estimate(&info, size, Target::Av1).unwrap().savings > e.savings);
    }

    #[test]
    fn lean_or_modern_files_are_skipped() {
        assert!(estimate(&h264(3_000_000), 3_000_000 / 8 * 7200, Target::Hevc).is_none());
        let mut hevc = h264(15_000_000);
        hevc.video.as_mut().unwrap().codec = "H.265".into();
        assert!(estimate(&hevc, 15_000_000 / 8 * 7200, Target::Hevc).is_none());
    }
}
