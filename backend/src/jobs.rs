//! Background job tracking with live progress broadcast (consumed by the SSE endpoint).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::db::now;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub library_id: Option<i64>,
    pub status: String,
    pub total: i64,
    pub processed: i64,
    pub failed: i64,
    pub skipped: i64,
    pub current_item: Option<String>,
    pub message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Job {
        job: JobSnapshot,
    },
    /// Media rows changed; clients should refresh lists/dashboards.
    MediaChanged,
    LibrariesChanged,
}

pub struct JobHandle {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub library_id: Option<i64>,
    pub started_at: String,
    pub cancel: CancellationToken,
    pub total: AtomicI64,
    pub processed: AtomicI64,
    pub failed: AtomicI64,
    pub skipped: AtomicI64,
    current: Mutex<Option<String>>,
    message: Mutex<Option<String>>,
}

impl JobHandle {
    pub fn set_total(&self, n: i64) {
        self.total.store(n, Ordering::Relaxed);
    }
    pub fn inc_processed(&self) {
        self.processed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::Relaxed);
    }
    pub fn set_current(&self, item: Option<String>) {
        *self.current.lock().unwrap() = item;
    }
    pub fn set_message(&self, msg: impl Into<String>) {
        *self.message.lock().unwrap() = Some(msg.into());
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn snapshot(&self, status: &str, finished_at: Option<String>) -> JobSnapshot {
        JobSnapshot {
            id: self.id,
            kind: self.kind.clone(),
            title: self.title.clone(),
            library_id: self.library_id,
            status: status.into(),
            total: self.total.load(Ordering::Relaxed),
            processed: self.processed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            current_item: self.current.lock().unwrap().clone(),
            message: self.message.lock().unwrap().clone(),
            started_at: self.started_at.clone(),
            finished_at,
        }
    }
}

#[derive(Clone)]
pub struct Jobs {
    pool: SqlitePool,
    tx: broadcast::Sender<Event>,
    running: Arc<Mutex<HashMap<i64, Arc<JobHandle>>>>,
}

impl Jobs {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        sqlx::query(
            "UPDATE jobs SET status = 'failed', message = 'Interrupted by a restart', finished_at = ?
             WHERE status IN ('queued', 'running')",
        )
        .bind(now())
        .execute(&pool)
        .await?;
        let (tx, _) = broadcast::channel(256);
        let jobs = Self { pool, tx, running: Default::default() };
        jobs.spawn_ticker();
        Ok(jobs)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    /// Returns the running job of the same kind (and library) if there is one.
    pub fn find_running(&self, kind: &str, library_id: Option<i64>) -> Option<Arc<JobHandle>> {
        self.running.lock().unwrap().values().find(|j| j.kind == kind && (library_id.is_none() || j.library_id == library_id)).cloned()
    }

    pub fn running(&self) -> Vec<JobSnapshot> {
        let mut v: Vec<_> = self.running.lock().unwrap().values().map(|j| j.snapshot("running", None)).collect();
        v.sort_by_key(|j| j.id);
        v
    }

    pub async fn start(&self, kind: &str, title: &str, library_id: Option<i64>) -> anyhow::Result<Arc<JobHandle>> {
        let started_at = now();
        let id: i64 =
            sqlx::query_scalar("INSERT INTO jobs (kind, title, library_id, status, started_at) VALUES (?, ?, ?, 'running', ?) RETURNING id")
                .bind(kind)
                .bind(title)
                .bind(library_id)
                .bind(&started_at)
                .fetch_one(&self.pool)
                .await?;
        let handle = Arc::new(JobHandle {
            id,
            kind: kind.into(),
            title: title.into(),
            library_id,
            started_at,
            cancel: CancellationToken::new(),
            total: AtomicI64::new(0),
            processed: AtomicI64::new(0),
            failed: AtomicI64::new(0),
            skipped: AtomicI64::new(0),
            current: Mutex::new(None),
            message: Mutex::new(None),
        });
        self.running.lock().unwrap().insert(id, handle.clone());
        self.emit(Event::Job { job: handle.snapshot("running", None) });
        tracing::info!(job = id, kind, title, "job started");
        Ok(handle)
    }

    pub async fn finish(&self, job: &JobHandle, result: anyhow::Result<()>) {
        let (status, msg) = match result {
            _ if job.is_cancelled() => ("cancelled", Some("Cancelled".to_string())),
            Ok(()) => ("completed", job.message.lock().unwrap().clone()),
            Err(e) => {
                tracing::error!(job = job.id, error = %format!("{e:#}"), "job failed");
                ("failed", Some(format!("{e:#}")))
            }
        };
        if let Some(m) = &msg {
            job.set_message(m.clone());
        }
        job.set_current(None);
        let snap = job.snapshot(status, Some(now()));
        let _ = sqlx::query(
            "UPDATE jobs SET status=?, total=?, processed=?, failed=?, skipped=?, current_item=NULL, message=?, finished_at=? WHERE id=?",
        )
        .bind(&snap.status)
        .bind(snap.total)
        .bind(snap.processed)
        .bind(snap.failed)
        .bind(snap.skipped)
        .bind(&snap.message)
        .bind(&snap.finished_at)
        .bind(job.id)
        .execute(&self.pool)
        .await;
        self.running.lock().unwrap().remove(&job.id);
        tracing::info!(job = job.id, status, processed = snap.processed, failed = snap.failed, "job finished");
        self.emit(Event::Job { job: snap });
    }

    pub fn cancel(&self, id: i64) -> bool {
        match self.running.lock().unwrap().get(&id) {
            Some(j) => {
                j.cancel.cancel();
                true
            }
            None => false,
        }
    }

    pub async fn record_failure(&self, job: &JobHandle, path: &str, error: &str, size: Option<i64>) {
        let _ = sqlx::query("INSERT INTO job_failures (job_id, path, error, size, failed_at) VALUES (?, ?, ?, ?, ?)")
            .bind(job.id)
            .bind(path)
            .bind(error)
            .bind(size)
            .bind(now())
            .execute(&self.pool)
            .await;
    }

    pub async fn history(&self, limit: i64) -> sqlx::Result<Vec<JobSnapshot>> {
        let mut rows: Vec<JobSnapshot> = sqlx::query_as("SELECT * FROM jobs ORDER BY id DESC LIMIT ?").bind(limit).fetch_all(&self.pool).await?;
        let live = self.running.lock().unwrap();
        for r in rows.iter_mut() {
            if let Some(h) = live.get(&r.id) {
                *r = h.snapshot("running", None);
            }
        }
        Ok(rows)
    }

    /// Broadcast progress of running jobs twice a second and persist it every few seconds.
    fn spawn_ticker(&self) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(500));
            let mut n: u64 = 0;
            loop {
                tick.tick().await;
                n += 1;
                let snaps = this.running();
                if snaps.is_empty() {
                    continue;
                }
                for s in &snaps {
                    this.emit(Event::Job { job: s.clone() });
                }
                if n.is_multiple_of(6) {
                    for s in snaps {
                        let _ = sqlx::query("UPDATE jobs SET total=?, processed=?, failed=?, skipped=?, current_item=? WHERE id=?")
                            .bind(s.total)
                            .bind(s.processed)
                            .bind(s.failed)
                            .bind(s.skipped)
                            .bind(&s.current_item)
                            .bind(s.id)
                            .execute(&this.pool)
                            .await;
                    }
                }
            }
        });
    }
}
