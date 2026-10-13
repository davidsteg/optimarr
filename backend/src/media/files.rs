//! Filesystem helpers: media discovery, MP4 fast-start detection, external subtitles.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::model::SubtitleTrack;
use super::probe::normalize_subtitle_format;

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "ts", "m2ts", "mts", "webm", "ogv", "wmv", "flv", "mpg", "mpeg",
];
pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "ass", "ssa", "sub", "idx", "sup"];

fn ext_lower(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

pub fn is_video(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // Skip hidden files, macOS resource forks and samples/trailers that are not library items.
    if name.starts_with('.') {
        return false;
    }
    ext_lower(path).is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.as_str()))
}

pub fn is_subtitle(path: &Path) -> bool {
    ext_lower(path).is_some_and(|e| SUBTITLE_EXTENSIONS.contains(&e.as_str()))
}

/// Walks the top-level MP4 boxes. Fast start means `moov` comes before `mdat`.
pub fn mp4_fast_start(path: &Path) -> std::io::Result<bool> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut offset = 0u64;
    let mut header = [0u8; 16];
    for _ in 0..64 {
        if offset + 8 > len {
            break;
        }
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(&mut header[..8])?;
        let mut size = u32::from_be_bytes(header[0..4].try_into().unwrap()) as u64;
        let kind = &header[4..8];
        match kind {
            b"moov" => return Ok(true),
            b"mdat" => return Ok(false),
            _ => {}
        }
        if size == 1 {
            f.read_exact(&mut header[8..16])?;
            size = u64::from_be_bytes(header[8..16].try_into().unwrap());
        } else if size == 0 {
            break;
        }
        if size < 8 {
            break;
        }
        offset += size;
    }
    Ok(false)
}

/// Finds external subtitles for `video` among `siblings` (file names in the same directory).
/// Follows the Jellyfin/Plex convention: `Movie.srt`, `Movie.en.srt`, `Movie.en.forced.srt`.
pub fn external_subtitles(video: &Path, siblings: &[String]) -> Vec<SubtitleTrack> {
    let Some(stem) = video.file_stem().and_then(|s| s.to_str()) else { return vec![] };
    let dir = video.parent().unwrap_or(Path::new(""));
    let stem_lower = stem.to_lowercase();
    let mut out: Vec<SubtitleTrack> = siblings
        .iter()
        .filter(|name| is_subtitle(Path::new(name.as_str())))
        .filter_map(|name| {
            let p = Path::new(name.as_str());
            let sub_stem = p.file_stem()?.to_str()?.to_lowercase();
            let rest = if sub_stem == stem_lower {
                ""
            } else {
                sub_stem.strip_prefix(&stem_lower)?.strip_prefix('.')?
            };
            let ext = ext_lower(p)?;
            // An .idx/.sub pair is one VobSub track; count it once.
            if ext == "sub" && siblings.iter().any(|s| s.eq_ignore_ascii_case(&name.replace(".sub", ".idx"))) {
                return None;
            }
            let format = match ext.as_str() {
                "sub" | "idx" => "VobSub".to_string(),
                "sup" => "PGS".to_string(),
                other => normalize_subtitle_format(other),
            };
            let tags: Vec<&str> = rest.split('.').filter(|t| !t.is_empty()).collect();
            Some(SubtitleTrack {
                format,
                language: tags.iter().find_map(|t| language_name(t)).unwrap_or_default(),
                embedded: false,
                forced: tags.iter().any(|t| *t == "forced"),
                path: Some(dir.join(name).to_string_lossy().into_owned()),
            })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

const LANGUAGES: &[(&str, &str, &str)] = &[
    ("en", "eng", "English"),
    ("de", "ger", "German"),
    ("de", "deu", "German"),
    ("fr", "fre", "French"),
    ("fr", "fra", "French"),
    ("es", "spa", "Spanish"),
    ("it", "ita", "Italian"),
    ("pt", "por", "Portuguese"),
    ("ru", "rus", "Russian"),
    ("ja", "jpn", "Japanese"),
    ("zh", "chi", "Chinese"),
    ("zh", "zho", "Chinese"),
    ("ko", "kor", "Korean"),
    ("ar", "ara", "Arabic"),
    ("hi", "hin", "Hindi"),
    ("nl", "dut", "Dutch"),
    ("nl", "nld", "Dutch"),
    ("pl", "pol", "Polish"),
    ("tr", "tur", "Turkish"),
    ("sv", "swe", "Swedish"),
    ("da", "dan", "Danish"),
    ("no", "nor", "Norwegian"),
    ("nb", "nob", "Norwegian Bokmål"),
    ("fi", "fin", "Finnish"),
    ("cs", "cze", "Czech"),
    ("cs", "ces", "Czech"),
    ("hu", "hun", "Hungarian"),
    ("ro", "rum", "Romanian"),
    ("ro", "ron", "Romanian"),
    ("th", "tha", "Thai"),
    ("vi", "vie", "Vietnamese"),
    ("he", "heb", "Hebrew"),
    ("el", "gre", "Greek"),
    ("el", "ell", "Greek"),
    ("ms", "may", "Malay"),
    ("ms", "msa", "Malay"),
    ("id", "ind", "Indonesian"),
    ("is", "ice", "Icelandic"),
    ("is", "isl", "Icelandic"),
    ("hr", "hrv", "Croatian"),
    ("uk", "ukr", "Ukrainian"),
];

/// Maps "en", "eng", "en-US" or "english" to a display name.
pub fn language_name(tag: &str) -> Option<String> {
    let t = tag.to_lowercase();
    let (base, region) = match t.split_once(['-', '_']) {
        Some((b, r)) => (b.to_string(), Some(r.to_uppercase())),
        None => (t.clone(), None),
    };
    let name = LANGUAGES.iter().find_map(|(two, three, name)| {
        (base == *two || base == *three || base == name.to_lowercase()).then_some(*name)
    })?;
    Some(match region {
        Some(r) if r.len() == 2 => format!("{name} ({base}-{r})"),
        _ => name.to_string(),
    })
}

/// Collect every media file under `root`, skipping unreadable directories.
pub fn discover(root: &Path) -> Vec<(PathBuf, std::fs::Metadata)> {
    walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden directories, Synology/QNAP metadata and recycle bins.
            e.depth() == 0 || !(name.starts_with('.') || name == "@eaDir" || name == "#recycle" || name == ".@__thumb")
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && is_video(e.path()))
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            Some((e.into_path(), md))
        })
        .collect()
}

pub fn mtime_secs(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn mp4_with(boxes: &[&[u8; 4]]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        for b in boxes {
            f.write_all(&16u32.to_be_bytes()).unwrap();
            f.write_all(*b).unwrap();
            f.write_all(&[0u8; 8]).unwrap();
        }
        f
    }

    #[test]
    fn fast_start_detection() {
        assert!(mp4_fast_start(mp4_with(&[b"ftyp", b"moov", b"mdat"]).path()).unwrap());
        assert!(!mp4_fast_start(mp4_with(&[b"ftyp", b"mdat", b"moov"]).path()).unwrap());
        assert!(mp4_fast_start(mp4_with(&[b"ftyp", b"free", b"moov"]).path()).unwrap());
    }

    #[test]
    fn finds_external_subs() {
        let siblings: Vec<String> = [
            "Movie (2020).mkv",
            "Movie (2020).en.srt",
            "Movie (2020).de.forced.srt",
            "Movie (2020).srt",
            "Movie (2020) - Extras.srt",
            "Other.srt",
            "Movie (2020).idx",
            "Movie (2020).sub",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let subs = external_subtitles(Path::new("/m/Movie (2020).mkv"), &siblings);
        assert_eq!(subs.len(), 4, "{subs:?}");
        let de = subs.iter().find(|s| s.language == "German").unwrap();
        assert!(de.forced);
        assert!(subs.iter().any(|s| s.format == "VobSub"));
    }

    #[test]
    fn languages() {
        assert_eq!(language_name("eng").as_deref(), Some("English"));
        assert_eq!(language_name("en-us").as_deref(), Some("English (en-US)"));
        assert_eq!(language_name("ger").as_deref(), Some("German"));
        assert_eq!(language_name("forced"), None);
    }
}
