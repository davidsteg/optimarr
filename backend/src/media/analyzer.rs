//! Probe + assess a single file.

use std::path::Path;

use super::files;
use super::model::{Assessment, MediaInfo};
use super::probe::{self, Backend};
use super::rating;
use crate::settings::{CompatibilityProfile, QualityProfile};

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Outcome {
    /// File was read; `broken` carries a reason when the data is unusable.
    Analyzed { info: MediaInfo, assessment: Assessment, broken: Option<String> },
    /// File could not be read at all (permissions, vanished). Not a property of the media.
    Failed(String),
}

pub struct Profiles<'a> {
    pub compat: &'a CompatibilityProfile,
    pub quality: &'a QualityProfile,
}

pub async fn analyze(path: &Path, size: u64, siblings: &[String], backends: &[Backend], profiles: &Profiles<'_>) -> Outcome {
    // Cheap readability check first so permission problems are reported as failures.
    if let Err(e) = tokio::fs::File::open(path).await {
        return Outcome::Failed(match e.kind() {
            std::io::ErrorKind::PermissionDenied => "Permission denied".into(),
            std::io::ErrorKind::NotFound => "File no longer exists".into(),
            _ => format!("Cannot open file: {e}"),
        });
    }
    if size == 0 {
        let info = MediaInfo { container: probe::container_from_extension(path), ..Default::default() };
        return analyzed(path, size, info, Some("File is empty (0 bytes)".into()), profiles);
    }

    let mut info = match probe::probe(path, backends).await {
        Ok(info) => info,
        Err(e) => {
            let info = MediaInfo { container: probe::container_from_extension(path), ..Default::default() };
            return analyzed(path, size, info, Some(format!("Unreadable media: {e:#}")), profiles);
        }
    };

    info.subtitles.extend(files::external_subtitles(path, siblings));
    if rating::is_mp4_family(&info.container) {
        let p = path.to_path_buf();
        info.fast_start = tokio::task::spawn_blocking(move || files::mp4_fast_start(&p).ok()).await.ok().flatten();
    }

    let broken = match &info.video {
        None => Some("No video stream found".to_string()),
        Some(v) if v.width == 0 && v.height == 0 && info.duration <= 0.0 => {
            Some("Media metadata is empty (file is probably truncated or corrupt)".into())
        }
        _ => None,
    };
    analyzed(path, size, info, broken, profiles)
}

fn analyzed(path: &Path, size: u64, info: MediaInfo, broken: Option<String>, profiles: &Profiles<'_>) -> Outcome {
    let assessment = rating::assess(&info, size, &path.to_string_lossy(), profiles.compat, profiles.quality);
    Outcome::Analyzed { info, assessment, broken }
}
