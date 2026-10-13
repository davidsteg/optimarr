use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

pub async fn open(path: &Path) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30))
        .pragma("temp_store", "memory")
        .pragma("mmap_size", "268435456");
    let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
pub async fn open_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use crate::settings::{self, GeneralSettings, QualityProfile};

    #[tokio::test]
    async fn settings_round_trip() {
        let pool = super::open_memory().await.unwrap();
        let mut g: GeneralSettings = settings::load(&pool).await.unwrap();
        assert_eq!(g.periodic_scan_hours, 12);
        g.periodic_scan_hours = 3;
        settings::save(&pool, &g).await.unwrap();
        let g2: GeneralSettings = settings::load(&pool).await.unwrap();
        assert_eq!(g2.periodic_scan_hours, 3);
        let q: QualityProfile = settings::load(&pool).await.unwrap();
        assert!(q.min_bits_per_pixel > 0.0);
    }
}
