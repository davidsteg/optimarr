//! Library scanning: discovery, incremental change detection, parallel analysis, batched writes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::db::now;
use crate::jobs::{Event, JobHandle};
use crate::media::analyzer::{self, Outcome, Profiles};
use crate::media::files;
use crate::media::model::{Assessment, MediaInfo};
use crate::repo::{self, NewAnalysis};
use crate::settings::{self, CompatibilityProfile, QualityProfile};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct WorkItem {
    pub path: PathBuf,
    pub library_id: Option<i64>,
    pub size: u64,
    pub mtime: i64,
}

impl WorkItem {
    pub fn from_path(path: PathBuf, library_id: Option<i64>) -> Option<Self> {
        let md = std::fs::metadata(&path).ok()?;
        Some(Self { size: md.len(), mtime: files::mtime_secs(&md), path, library_id })
    }
}

/// Start a scan job for one library. `full` re-analyzes every file instead of only changed ones.
pub async fn start_library_scan(state: &AppState, library_id: i64, full: bool) -> Result<i64> {
    if let Some(j) = state.jobs.find_running("scan", Some(library_id)) {
        return Ok(j.id);
    }
    let lib = repo::get_library(&state.pool, library_id).await?.context("library not found")?;
    let job = state.jobs.start("scan", &format!("{} {}", if full { "Full scan of" } else { "Scan" }, lib.name), Some(lib.id)).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = scan_library(&st, &job, &lib.path, lib.id, full).await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
        st.jobs.emit(Event::LibrariesChanged);
    });
    Ok(id)
}

async fn scan_library(state: &AppState, job: &Arc<JobHandle>, root: &str, library_id: i64, full: bool) -> Result<()> {
    let root_path = PathBuf::from(root);
    if !tokio::fs::metadata(&root_path).await.map(|m| m.is_dir()).unwrap_or(false) {
        bail!("Library folder {root} does not exist or is not readable (is the volume mounted?)");
    }
    job.set_message("Discovering files");
    let rp = root_path.clone();
    let discovered = tokio::task::spawn_blocking(move || files::discover(&rp)).await?;

    let existing: Vec<(i64, String, i64, i64, String)> =
        sqlx::query_as("SELECT id, path, size, mtime, status FROM media_files WHERE library_id = ?").bind(library_id).fetch_all(&state.pool).await?;
    let known: HashMap<&str, (i64, i64, &str)> = existing.iter().map(|(_, p, s, m, st)| (p.as_str(), (*s, *m, st.as_str()))).collect();

    let mut work = Vec::new();
    let mut seen = HashSet::with_capacity(discovered.len());
    for (path, md) in discovered {
        let p = path.to_string_lossy().into_owned();
        let size = md.len();
        let mtime = files::mtime_secs(&md);
        let unchanged = matches!(known.get(p.as_str()), Some((s, m, st)) if *s == size as i64 && *m == mtime && *st == "ok");
        if unchanged && !full {
            job.inc_skipped();
        } else {
            work.push(WorkItem { path, library_id: Some(library_id), size, mtime });
        }
        seen.insert(p);
    }

    // Files that disappeared are flagged, not deleted, so a temporarily missing mount can't wipe data.
    let missing: Vec<i64> = existing.iter().filter(|(_, p, _, _, st)| !seen.contains(p) && st != "missing").map(|(id, ..)| *id).collect();
    if !missing.is_empty() {
        let mut tx = state.pool.begin().await?;
        for id in &missing {
            sqlx::query("UPDATE media_files SET status = 'missing', status_reason = 'File no longer exists on disk' WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    // Files that came back.
    sqlx::query("UPDATE media_files SET status = 'ok', status_reason = NULL WHERE library_id = ? AND status = 'missing' AND path IN (SELECT value FROM json_each(?))")
        .bind(library_id)
        .bind(serde_json::to_string(&seen.iter().collect::<Vec<_>>())?)
        .execute(&state.pool)
        .await?;

    job.set_total(work.len() as i64 + job.skipped.load(std::sync::atomic::Ordering::Relaxed));
    job.set_message(format!(
        "{} new or changed, {} unchanged, {} missing",
        work.len(),
        job.skipped.load(std::sync::atomic::Ordering::Relaxed),
        missing.len()
    ));
    analyze_items(state, job, work).await?;

    sqlx::query("UPDATE libraries SET last_scanned_at = ? WHERE id = ?").bind(now()).bind(library_id).execute(&state.pool).await?;
    Ok(())
}

enum WriteMsg {
    Analyzed { item: WorkItem, info: MediaInfo, assessment: Assessment, broken: Option<String> },
}

/// Analyze items in parallel (bounded by the global probe semaphore) and persist results in batches.
pub async fn analyze_items(state: &AppState, job: &Arc<JobHandle>, items: Vec<WorkItem>) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let backends = state.probe_backends().await;
    let compat: CompatibilityProfile = settings::load(&state.pool).await?;
    let quality: QualityProfile = settings::load(&state.pool).await?;

    // One directory listing per folder instead of one per file.
    let dirs: HashSet<PathBuf> = items.iter().filter_map(|i| i.path.parent().map(Path::to_path_buf)).collect();
    let siblings: HashMap<PathBuf, Vec<String>> = tokio::task::spawn_blocking(move || {
        dirs.into_iter()
            .map(|d| {
                let names = std::fs::read_dir(&d)
                    .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                    .unwrap_or_default();
                (d, names)
            })
            .collect()
    })
    .await?;
    let siblings = Arc::new(siblings);

    let (tx, rx) = mpsc::channel::<WriteMsg>(256);
    let writer = tokio::spawn(write_results(state.clone(), job.clone(), rx));

    let concurrency = state.probe_slots.available_permits().max(1) * 2;
    let compat = Arc::new(compat);
    let quality = Arc::new(quality);
    let backends = Arc::new(backends);
    futures::stream::iter(items)
        .take_while(|_| futures::future::ready(!job.is_cancelled()))
        .map(|item| {
            let state = state.clone();
            let job = job.clone();
            let tx = tx.clone();
            let siblings = siblings.clone();
            let (compat, quality, backends) = (compat.clone(), quality.clone(), backends.clone());
            async move {
                let Ok(_permit) = state.probe_slots.acquire().await else { return };
                if job.is_cancelled() {
                    return;
                }
                job.set_current(Some(item.path.to_string_lossy().into_owned()));
                let empty = Vec::new();
                let sib = item.path.parent().and_then(|d| siblings.get(d)).unwrap_or(&empty);
                let profiles = Profiles { compat: &compat, quality: &quality };
                match analyzer::analyze(&item.path, item.size, sib, &backends, &profiles).await {
                    Outcome::Analyzed { info, assessment, broken } => {
                        let _ = tx.send(WriteMsg::Analyzed { item, info, assessment, broken }).await;
                    }
                    Outcome::Failed(err) => {
                        job.inc_failed();
                        job.inc_processed();
                        let p = item.path.to_string_lossy();
                        tracing::warn!(path = %p, error = %err, "analysis failed");
                        state.jobs.record_failure(&job, &p, &err, Some(item.size as i64)).await;
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<()>()
        .await;
    drop(tx);
    writer.await??;
    Ok(())
}

async fn write_results(state: AppState, job: Arc<JobHandle>, mut rx: mpsc::Receiver<WriteMsg>) -> Result<()> {
    const BATCH: usize = 64;
    let mut buf = Vec::with_capacity(BATCH);
    let mut last_emit = std::time::Instant::now();
    loop {
        let timed_out = match tokio::time::timeout(Duration::from_millis(750), rx.recv()).await {
            Ok(Some(m)) => {
                buf.push(m);
                false
            }
            Ok(None) => break,
            Err(_) => true,
        };
        if buf.len() >= BATCH || (timed_out && !buf.is_empty()) {
            flush(&state, &job, &mut buf).await?;
            if last_emit.elapsed() > Duration::from_secs(3) {
                state.jobs.emit(Event::MediaChanged);
                last_emit = std::time::Instant::now();
            }
        }
    }
    flush(&state, &job, &mut buf).await
}

async fn flush(state: &AppState, job: &JobHandle, buf: &mut Vec<WriteMsg>) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    for WriteMsg::Analyzed { item, info, assessment, broken } in buf.iter() {
        let path = item.path.to_string_lossy();
        repo::upsert_analysis(
            &mut *tx,
            &NewAnalysis {
                library_id: item.library_id,
                path: &path,
                size: item.size as i64,
                mtime: item.mtime,
                info,
                assessment,
                broken: broken.as_deref(),
            },
        )
        .await?;
        job.inc_processed();
    }
    tx.commit().await?;
    buf.clear();
    Ok(())
}

/// Re-analyze specific media rows (by id) as a job.
pub async fn start_rescan(state: &AppState, ids: Vec<i64>, title: &str) -> Result<i64> {
    let job = state.jobs.start("rescan", title, None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = async {
            let mut items = Vec::new();
            for chunk in ids.chunks(500) {
                let sql =
                    format!("SELECT id, path, library_id FROM media_files WHERE id IN ({})", chunk.iter().map(|_| "?").collect::<Vec<_>>().join(","));
                let mut q = sqlx::query_as::<_, (i64, String, Option<i64>)>(&sql);
                for id in chunk {
                    q = q.bind(id);
                }
                for (row_id, path, lib) in q.fetch_all(&st.pool).await? {
                    match WorkItem::from_path(PathBuf::from(&path), lib) {
                        Some(w) => items.push(w),
                        None => {
                            sqlx::query("UPDATE media_files SET status='missing', status_reason='File no longer exists on disk' WHERE id=?")
                                .bind(row_id)
                                .execute(&st.pool)
                                .await?;
                            job.inc_failed();
                            st.jobs.record_failure(&job, &path, "File no longer exists", None).await;
                        }
                    }
                }
            }
            job.set_total(items.len() as i64 + job.failed.load(std::sync::atomic::Ordering::Relaxed));
            analyze_items(&st, &job, items).await
        }
        .await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(id)
}

/// Analyze arbitrary paths (watcher, webhook). Paths outside every library are ignored.
pub async fn start_path_analysis(state: &AppState, paths: Vec<PathBuf>, title: &str) -> Result<Option<i64>> {
    let mut items = Vec::new();
    for p in paths {
        let s = p.to_string_lossy().into_owned();
        let Some(lib) = repo::library_for_path(&state.pool, &s).await? else {
            tracing::debug!(path = %s, "ignoring path outside libraries");
            continue;
        };
        if let Some(w) = WorkItem::from_path(p, Some(lib.id)) {
            items.push(w);
        }
    }
    if items.is_empty() {
        return Ok(None);
    }
    let job = state.jobs.start("rescan", title, None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        job.set_total(items.len() as i64);
        let res = analyze_items(&st, &job, items).await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(Some(id))
}

/// Recompute scores for every analyzed file from stored data (after profile changes). No probing.
pub async fn rescore_all(state: &AppState) -> Result<i64> {
    if let Some(j) = state.jobs.find_running("rescore", None) {
        return Ok(j.id);
    }
    let job = state.jobs.start("rescore", "Recalculate scores", None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = async {
            let compat: CompatibilityProfile = settings::load(&st.pool).await?;
            let quality: QualityProfile = settings::load(&st.pool).await?;
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files").fetch_one(&st.pool).await?;
            job.set_total(total);
            let mut last_id = 0i64;
            loop {
                let rows: Vec<repo::MediaFile> =
                    sqlx::query_as("SELECT * FROM media_files WHERE id > ? ORDER BY id LIMIT 500").bind(last_id).fetch_all(&st.pool).await?;
                if rows.is_empty() || job.is_cancelled() {
                    break;
                }
                let mut tx = st.pool.begin().await?;
                for r in &rows {
                    let info = r.to_media_info();
                    let a = crate::media::rating::assess(&info, r.size as u64, &r.path, &compat, &quality);
                    repo::update_scores(&mut *tx, r.id, &a).await?;
                    job.inc_processed();
                }
                tx.commit().await?;
                last_id = rows.last().unwrap().id;
            }
            Ok(())
        }
        .await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(id)
}
