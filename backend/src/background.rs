//! Long-running background tasks: library file watcher, periodic scans, processing re-checks.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::jobs::Event;
use crate::media::files;
use crate::scanner;
use crate::settings::{self, GeneralSettings};
use crate::state::AppState;

pub fn spawn_all(state: AppState) {
    spawn_watcher(state.clone());
    spawn_scheduler(state.clone());
    crate::integrations::jellyfin::spawn_background(state);
}

/// Watches enabled libraries and analyzes new/changed files once they stop growing.
fn spawn_watcher(state: AppState) {
    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<notify::Event>>();
        let mut events = state.jobs.subscribe();
        let mut watcher: Option<notify::RecommendedWatcher> = None;
        let mut watched: Vec<PathBuf> = Vec::new();
        let mut pending: HashMap<PathBuf, (Instant, u64)> = HashMap::new();
        let mut rearm = true;
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            if rearm {
                rearm = false;
                let general: GeneralSettings = settings::load(&state.pool).await.unwrap_or_default();
                let paths: Vec<PathBuf> = if general.watch_libraries {
                    sqlx::query_scalar::<_, String>("SELECT path FROM libraries WHERE enabled = 1 AND watch = 1")
                        .fetch_all(&state.pool)
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .map(PathBuf::from)
                        .collect()
                } else {
                    vec![]
                };
                if paths != watched {
                    watcher = None;
                    if !paths.is_empty() {
                        let tx = tx.clone();
                        match notify::recommended_watcher(move |res| {
                            let _ = tx.send(res);
                        }) {
                            Ok(mut w) => {
                                for p in &paths {
                                    if let Err(e) = w.watch(p, RecursiveMode::Recursive) {
                                        tracing::warn!(path = %p.display(), error = %e, "cannot watch library");
                                    }
                                }
                                tracing::info!(count = paths.len(), "watching libraries for changes");
                                watcher = Some(w);
                            }
                            Err(e) => tracing::warn!(error = %e, "file watching unavailable"),
                        }
                    }
                    watched = paths;
                }
            }
            tokio::select! {
                Some(res) = rx.recv() => {
                    let Ok(ev) = res else { continue };
                    use notify::EventKind::*;
                    match ev.kind {
                        Create(_) | Modify(_) => {
                            for p in ev.paths.into_iter().filter(|p| files::is_video(p)) {
                                pending.insert(p, (Instant::now(), u64::MAX));
                            }
                        }
                        Remove(_) => {
                            for p in ev.paths.into_iter().filter(|p| files::is_video(p)) {
                                pending.remove(&p);
                                let _ = sqlx::query("UPDATE media_files SET status='missing', status_reason='File was deleted' WHERE path = ?")
                                    .bind(p.to_string_lossy().as_ref())
                                    .execute(&state.pool)
                                    .await;
                                state.jobs.emit(Event::MediaChanged);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(ev) = events.recv() => {
                    if matches!(ev, Event::LibrariesChanged) {
                        rearm = true;
                    }
                }
                _ = tick.tick() => {
                    // A file is ready when it has not changed for 30s and its size is stable.
                    let mut ready = Vec::new();
                    pending.retain(|p, (last, size)| {
                        let Ok(md) = std::fs::metadata(p) else { return false };
                        if md.len() != *size {
                            *size = md.len();
                            *last = Instant::now();
                            return true;
                        }
                        if last.elapsed() >= Duration::from_secs(30) {
                            ready.push(p.clone());
                            return false;
                        }
                        true
                    });
                    if !ready.is_empty() {
                        let n = ready.len();
                        if let Err(e) = scanner::start_path_analysis(&state, ready, &format!("Analyze {n} new or changed file(s)")).await {
                            tracing::warn!(error = %e, "auto analysis failed");
                        }
                    }
                    let _ = &watcher;
                }
            }
        }
    });
}

/// Periodic incremental scans and re-checks of files waiting for a replacement.
fn spawn_scheduler(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15 * 60));
        tick.tick().await;
        loop {
            tick.tick().await;
            let general: GeneralSettings = settings::load(&state.pool).await.unwrap_or_default();

            // Libraries with files that were sent for redownload long enough ago.
            let cutoff = (chrono::Utc::now() - chrono::Duration::hours(general.processing_rescan_hours.max(1)))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let mut libs: Vec<i64> = sqlx::query_scalar(
                "SELECT DISTINCT library_id FROM media_files WHERE status = 'processing' AND processing_since < ? AND library_id IS NOT NULL",
            )
            .bind(&cutoff)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();

            if general.periodic_scan_hours > 0 {
                let cutoff = (chrono::Utc::now() - chrono::Duration::hours(general.periodic_scan_hours as i64))
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                let due: Vec<i64> = sqlx::query_scalar(
                    "SELECT id FROM libraries WHERE enabled = 1 AND (last_scanned_at IS NULL OR last_scanned_at < ?)",
                )
                .bind(cutoff)
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default();
                libs.extend(due);
            }
            libs.sort();
            libs.dedup();
            for id in libs {
                if let Err(e) = scanner::start_library_scan(&state, id, false).await {
                    tracing::warn!(library = id, error = %e, "scheduled scan failed to start");
                }
            }
        }
    });
}
