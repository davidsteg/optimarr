use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use crate::jobs::Jobs;
use crate::media::probe::{Backend, Probers};
use crate::settings::{self, GeneralSettings};

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    /// API key required for /api and /mcp. Empty disables auth.
    pub api_key: String,
    pub version: &'static str,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jobs: Jobs,
    pub probers: Probers,
    pub http: reqwest::Client,
    pub config: Arc<Config>,
    /// Global cap on concurrent probe processes across all jobs.
    pub probe_slots: Arc<Semaphore>,
    pub fixes: crate::fixer::FixQueue,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub async fn new(pool: SqlitePool, config: Config) -> anyhow::Result<Self> {
        let jobs = Jobs::new(pool.clone()).await?;
        let probers = Probers::detect().await;
        match (&probers.mediainfo, &probers.ffprobe) {
            (None, None) => tracing::error!("neither mediainfo nor ffprobe found in PATH: analysis will fail"),
            (mi, ff) => tracing::info!(mediainfo = ?mi, ffprobe = ?ff, "probe tools detected"),
        }
        let general: GeneralSettings = settings::load(&pool).await.unwrap_or_default();
        let slots = if general.scan_concurrency == 0 {
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 16)
        } else {
            general.scan_concurrency
        };
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("Optimarr/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            pool,
            jobs,
            probers,
            http,
            config: Arc::new(config),
            probe_slots: Arc::new(Semaphore::new(slots)),
            fixes: Default::default(),
            started_at: chrono::Utc::now(),
        })
    }

    pub async fn probe_backends(&self) -> Vec<Backend> {
        let general: GeneralSettings = settings::load(&self.pool).await.unwrap_or_default();
        self.probers.order(&general.probe_backend)
    }
}
