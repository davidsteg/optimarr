//! Fix queue: runs the suggested ffmpeg command for a file, one file at a time, then swaps the
//! result in and re-analyzes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::db::now;
use crate::integrations::servarr::{Client, Kind};
use crate::jobs::Event;
use crate::media::{fixes, rating};
use crate::repo;
use crate::settings::{self, CompatibilityProfile, QualityProfile, SettingsDoc};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct FixSettings {
    /// The worker only runs when enabled (the media folders must be writable).
    pub enabled: bool,
    /// Swap the fixed file in for the original. Otherwise the result is left next to it.
    pub replace_original: bool,
    /// When replacing, keep the original as `<name>.orig` instead of deleting it.
    pub keep_original: bool,
}

impl Default for FixSettings {
    fn default() -> Self {
        Self { enabled: false, replace_original: true, keep_original: true }
    }
}

impl SettingsDoc for FixSettings {
    const KEY: &'static str = "fixes";
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub id: i64,
    pub media_file_id: Option<i64>,
    pub path: String,
    pub status: String,
    pub command: String,
    #[serde(skip)]
    pub args: Json<Vec<String>>,
    pub output_path: String,
    pub final_path: Option<String>,
    pub progress: f64,
    pub error: Option<String>,
    pub size_before: Option<i64>,
    pub size_after: Option<i64>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

/// Wakes the worker and tracks the running fix so it can be cancelled.
#[derive(Clone, Default)]
pub struct FixQueue {
    wake: Arc<Notify>,
    running: Arc<Mutex<HashMap<i64, CancellationToken>>>,
}

impl FixQueue {
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub fn cancel(&self, id: i64) -> bool {
        match self.running.lock().unwrap().get(&id) {
            Some(t) => {
                t.cancel();
                true
            }
            None => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EnqueueError {
    #[error("No file with that id")]
    NotFound,
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<sqlx::Error> for EnqueueError {
    fn from(e: sqlx::Error) -> Self {
        Self::Other(e.into())
    }
}

/// Queue the suggested fix for a file.
pub async fn enqueue(state: &AppState, media_id: i64) -> Result<Fix, EnqueueError> {
    let file = repo::get_media(&state.pool, media_id).await?.ok_or(EnqueueError::NotFound)?;
    if file.status != "ok" {
        return Err(EnqueueError::Invalid(format!("The file is {}; only readable files can be fixed", file.status)));
    }
    let active: Option<i64> = sqlx::query_scalar("SELECT id FROM fixes WHERE media_file_id = ? AND status IN ('queued', 'running')")
        .bind(media_id)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(id) = active {
        return Err(EnqueueError::Invalid(format!("A fix for this file is already queued (#{id})")));
    }
    let compat: CompatibilityProfile = settings::load(&state.pool).await?;
    let quality: QualityProfile = settings::load(&state.pool).await?;
    let info = file.to_media_info();
    let a = rating::assess(&info, file.size as u64, &file.path, &compat, &quality);
    let plan = fixes::ffmpeg_plan_for(&info, &compat, &a.issues, &file.path)
        .ok_or_else(|| EnqueueError::Invalid("Nothing to fix: no compatibility issue that ffmpeg can solve".into()))?;
    let command = plan.command(&file.path);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO fixes (media_file_id, path, status, command, args, output_path, size_before, created_at)
         VALUES (?, ?, 'queued', ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(media_id)
    .bind(&file.path)
    .bind(&command)
    .bind(Json(&plan.args))
    .bind(&plan.output)
    .bind(file.size)
    .bind(now())
    .fetch_one(&state.pool)
    .await?;
    state.fixes.wake();
    state.jobs.emit(Event::FixesChanged);
    Ok(get(state, id).await?.expect("just inserted"))
}

pub async fn get(state: &AppState, id: i64) -> sqlx::Result<Option<Fix>> {
    sqlx::query_as("SELECT * FROM fixes WHERE id = ?").bind(id).fetch_optional(&state.pool).await
}

pub async fn list(state: &AppState, status: Option<&str>, limit: i64) -> sqlx::Result<Vec<Fix>> {
    sqlx::query_as(
        "SELECT * FROM fixes WHERE (?1 IS NULL OR status = ?1)
         ORDER BY CASE status WHEN 'running' THEN 0 WHEN 'queued' THEN 1 ELSE 2 END, id DESC LIMIT ?2",
    )
    .bind(status)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
}

/// Cancel a queued or running fix.
pub async fn cancel(state: &AppState, id: i64) -> sqlx::Result<bool> {
    if state.fixes.cancel(id) {
        return Ok(true);
    }
    let r = sqlx::query("UPDATE fixes SET status = 'cancelled', finished_at = ? WHERE id = ? AND status = 'queued'")
        .bind(now())
        .bind(id)
        .execute(&state.pool)
        .await?;
    state.jobs.emit(Event::FixesChanged);
    Ok(r.rows_affected() > 0)
}

pub async fn ffmpeg_available() -> bool {
    tokio::process::Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().await.is_ok_and(|s| s.success())
}

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        // Fixes interrupted by a restart start over.
        let _ = sqlx::query("UPDATE fixes SET status = 'queued', progress = 0 WHERE status = 'running'").execute(&state.pool).await;
        loop {
            let s: FixSettings = settings::load(&state.pool).await.unwrap_or_default();
            let next: Option<Fix> = if s.enabled {
                sqlx::query_as("SELECT * FROM fixes WHERE status = 'queued' ORDER BY id LIMIT 1").fetch_optional(&state.pool).await.unwrap_or(None)
            } else {
                None
            };
            match next {
                Some(fix) => run(&state, fix, &s).await,
                None => {
                    let _ = tokio::time::timeout(Duration::from_secs(30), state.fixes.wake.notified()).await;
                }
            }
        }
    });
}

async fn run(state: &AppState, fix: Fix, s: &FixSettings) {
    let token = CancellationToken::new();
    state.fixes.running.lock().unwrap().insert(fix.id, token.clone());
    let _ = sqlx::query("UPDATE fixes SET status = 'running', started_at = ?, progress = 0, error = NULL WHERE id = ?")
        .bind(now())
        .bind(fix.id)
        .execute(&state.pool)
        .await;
    state.jobs.emit(Event::FixesChanged);
    tracing::info!(fix = fix.id, path = %fix.path, "running fix");

    let res = execute(state, &fix, s, &token).await;
    state.fixes.running.lock().unwrap().remove(&fix.id);
    let (status, error, final_path, size_after) = match res {
        Ok((final_path, size)) => ("done", None, Some(final_path), Some(size)),
        Err(_) if token.is_cancelled() => ("cancelled", None, None, None),
        Err(e) => ("failed", Some(format!("{e:#}")), None, None),
    };
    if status != "done" {
        let _ = tokio::fs::remove_file(&fix.output_path).await;
    }
    let _ = sqlx::query("UPDATE fixes SET status = ?, error = ?, final_path = ?, size_after = ?, finished_at = ?, progress = ? WHERE id = ?")
        .bind(status)
        .bind(&error)
        .bind(&final_path)
        .bind(size_after)
        .bind(now())
        .bind(if status == "done" { 1.0 } else { 0.0 })
        .bind(fix.id)
        .execute(&state.pool)
        .await;
    state.jobs.emit(Event::FixesChanged);
    tracing::info!(fix = fix.id, status, error = ?error, "fix finished");

    let n: crate::notify::NotificationSettings = settings::load(&state.pool).await.unwrap_or_default();
    if n.on_fix_finished && status != "cancelled" {
        let name = Path::new(&fix.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (title, msg) = match &error {
            None => (
                format!("Fixed {name}"),
                format!("{} → {}", crate::notify::human_size(fix.size_before.unwrap_or(0)), crate::notify::human_size(size_after.unwrap_or(0))),
            ),
            Some(e) => (format!("Fix failed for {name}"), e.chars().take(500).collect()),
        };
        let event = if error.is_none() { "fix_done" } else { "fix_failed" };
        crate::notify::send(state, event, &title, &msg, json!({ "fixId": fix.id, "fileId": fix.media_file_id })).await;
    }
}

/// Run ffmpeg, then swap the result in. Returns the final path and its size.
async fn execute(state: &AppState, fix: &Fix, s: &FixSettings, cancel: &CancellationToken) -> Result<(String, i64)> {
    if !ffmpeg_available().await {
        bail!("ffmpeg is not installed");
    }
    let input = PathBuf::from(&fix.path);
    if !tokio::fs::try_exists(&input).await.unwrap_or(false) {
        bail!("The file no longer exists");
    }
    let duration: f64 = match fix.media_file_id {
        Some(id) => repo::get_media(&state.pool, id).await?.map(|m| m.duration).unwrap_or(0.0),
        None => 0.0,
    };

    let mut child = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-nostdin", "-nostats", "-loglevel", "error", "-progress", "pipe:1"])
        .args(fix.args.0.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("cannot start ffmpeg")?;
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });

    let mut lines = BufReader::new(stdout).lines();
    let mut last_write = Instant::now();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.kill().await;
                bail!("Cancelled");
            }
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break };
                // out_time_us is in microseconds (out_time_ms is too, despite its name).
                let us = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms="));
                if let (Some(us), true) = (us.and_then(|v| v.trim().parse::<f64>().ok()), duration > 0.0) {
                    if last_write.elapsed() > Duration::from_secs(2) {
                        last_write = Instant::now();
                        let p = (us / 1e6 / duration).clamp(0.0, 0.99);
                        let _ = sqlx::query("UPDATE fixes SET progress = ? WHERE id = ?").bind(p).bind(fix.id).execute(&state.pool).await;
                        state.jobs.emit(Event::FixesChanged);
                    }
                }
            }
        }
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        let tail: Vec<&str> = stderr.lines().rev().take(6).collect();
        bail!("ffmpeg failed ({status}): {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
    }

    let output = PathBuf::from(&fix.output_path);
    let out_size = tokio::fs::metadata(&output).await.context("ffmpeg produced no output")?.len();
    if out_size == 0 {
        bail!("ffmpeg produced an empty file");
    }
    let final_path = if s.replace_original { swap_in(&input, &output, s.keep_original).await? } else { output.clone() };
    let final_str = final_path.to_string_lossy().into_owned();

    if let Some(id) = fix.media_file_id {
        // Keep the same row (and its Sonarr/Radarr link and playback history) for the new file.
        if final_str != fix.path {
            let name = final_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            sqlx::query("UPDATE media_files SET path = ?, file_name = ? WHERE id = ? AND NOT EXISTS (SELECT 1 FROM media_files WHERE path = ?)")
                .bind(&final_str)
                .bind(&name)
                .bind(id)
                .bind(&final_str)
                .execute(&state.pool)
                .await?;
        }
        if let Some(m) = repo::get_media(&state.pool, id).await? {
            tell_servarr(state, &m).await;
        }
    }
    let _ = crate::scanner::start_path_analysis(state, vec![final_path.clone()], "Analyze fixed file").await;
    Ok((final_str, out_size as i64))
}

/// Replace `input` with `output` (whose extension may differ). Returns the final path.
async fn swap_in(input: &Path, output: &Path, keep_original: bool) -> Result<PathBuf> {
    let ext = output.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let final_path = input.with_extension(&ext);
    if keep_original {
        let mut backup = input.as_os_str().to_owned();
        backup.push(".orig");
        tokio::fs::rename(input, &backup).await.context("cannot rename the original (is the media folder writable?)")?;
    } else if final_path != input {
        tokio::fs::remove_file(input).await.context("cannot remove the original (is the media folder writable?)")?;
    }
    tokio::fs::rename(output, &final_path).await.context("cannot move the fixed file into place")?;
    Ok(final_path)
}

/// Ask Sonarr/Radarr to rescan so they pick up a renamed file.
async fn tell_servarr(state: &AppState, m: &repo::MediaFile) {
    let (kind, body) = match (m.servarr_kind.as_deref(), m.sonarr_series_id, m.radarr_movie_id) {
        (Some("sonarr"), Some(id), _) => (Kind::Sonarr, json!({ "name": "RescanSeries", "seriesId": id })),
        (Some("radarr"), _, Some(id)) => (Kind::Radarr, json!({ "name": "RescanMovie", "movieId": id })),
        _ => return,
    };
    match Client::from_state(state, kind).await {
        Ok(c) => {
            if let Err(e) = c.command(body).await {
                tracing::warn!(error = %format!("{e:#}"), "cannot ask {} to rescan", kind.label());
            }
        }
        Err(e) => tracing::debug!(error = %format!("{e:#}"), "servarr not configured"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn swaps_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("Movie.mkv");
        let output = dir.path().join("Movie.optimized.mp4");
        tokio::fs::write(&input, b"old").await.unwrap();
        tokio::fs::write(&output, b"new").await.unwrap();
        let f = swap_in(&input, &output, true).await.unwrap();
        assert_eq!(f, dir.path().join("Movie.mp4"));
        assert_eq!(tokio::fs::read(&f).await.unwrap(), b"new");
        assert!(dir.path().join("Movie.mkv.orig").exists());
        assert!(!input.exists());
    }

    #[tokio::test]
    async fn swaps_same_extension_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("Movie.mp4");
        let output = dir.path().join("Movie.optimized.mp4");
        tokio::fs::write(&input, b"old").await.unwrap();
        tokio::fs::write(&output, b"new").await.unwrap();
        let f = swap_in(&input, &output, false).await.unwrap();
        assert_eq!(f, input);
        assert_eq!(tokio::fs::read(&f).await.unwrap(), b"new");
        assert!(!output.exists());
    }
}
