//! Upgrade finder: asks Sonarr/Radarr to search for a better release of low-quality files.
//!
//! Unlike a redownload, the current file is kept. Sonarr/Radarr only grab a release that their
//! quality profile ranks higher than the file they already have.

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::now;
use crate::integrations::servarr::{Client, Kind};
use crate::repo::MediaFile;
use crate::settings::{self, SettingsDoc};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UpgradeSettings {
    /// Files at or below this quality score are candidates.
    pub max_quality_score: i64,
    /// Do not search the same file again within this many days.
    pub cooldown_days: i64,
    /// Search automatically for the worst candidates.
    pub auto_enabled: bool,
    pub auto_interval_hours: i64,
    pub auto_batch_size: i64,
}

impl Default for UpgradeSettings {
    fn default() -> Self {
        Self { max_quality_score: 60, cooldown_days: 14, auto_enabled: false, auto_interval_hours: 24, auto_batch_size: 10 }
    }
}

impl SettingsDoc for UpgradeSettings {
    const KEY: &'static str = "upgrades";
}

/// Files managed by Sonarr/Radarr whose quality is at or below `max_score`, worst first.
pub async fn candidates(state: &AppState, max_score: i64, include_recent: bool, limit: i64) -> Result<(i64, Vec<MediaFile>)> {
    let s: UpgradeSettings = settings::load(&state.pool).await?;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(s.cooldown_days.max(0))).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let filter = "status = 'ok' AND quality_verdict != 'unknown' AND quality_score <= ?1
        AND ((servarr_kind = 'sonarr' AND sonarr_episode_id IS NOT NULL) OR (servarr_kind = 'radarr' AND radarr_movie_id IS NOT NULL))
        AND (?2 OR upgrade_requested_at IS NULL OR upgrade_requested_at < ?3)";
    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM media_files WHERE {filter}"))
        .bind(max_score)
        .bind(include_recent)
        .bind(&cutoff)
        .fetch_one(&state.pool)
        .await?;
    let rows = sqlx::query_as(&format!("SELECT * FROM media_files WHERE {filter} ORDER BY quality_score, compat_score LIMIT ?4"))
        .bind(max_score)
        .bind(include_recent)
        .bind(&cutoff)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?;
    Ok((total, rows))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: i64,
    pub title: String,
    pub success: bool,
    pub message: String,
}

/// Start a search in Sonarr/Radarr for each file. Episodes of one series are batched.
pub async fn search(state: &AppState, ids: &[i64]) -> Result<Vec<SearchResult>> {
    let mut clients: HashMap<Kind, Option<Client>> = HashMap::new();
    let mut out = Vec::new();
    for &id in ids {
        let Some(f) = crate::repo::get_media(&state.pool, id).await? else {
            out.push(SearchResult { id, title: String::new(), success: false, message: "File not found".into() });
            continue;
        };
        let title = f.title();
        let (kind, body) = match (f.servarr_kind.as_deref(), f.sonarr_episode_id, f.radarr_movie_id) {
            (Some("sonarr"), Some(ep), _) => (Kind::Sonarr, json!({ "name": "EpisodeSearch", "episodeIds": [ep] })),
            (Some("radarr"), _, Some(movie)) => (Kind::Radarr, json!({ "name": "MoviesSearch", "movieIds": [movie] })),
            _ => {
                out.push(SearchResult { id, title, success: false, message: "Not linked to Sonarr or Radarr (run a match first)".into() });
                continue;
            }
        };
        if let std::collections::hash_map::Entry::Vacant(e) = clients.entry(kind) {
            e.insert(Client::from_state(state, kind).await.ok());
        }
        let Some(Some(client)) = clients.get(&kind) else {
            out.push(SearchResult { id, title, success: false, message: format!("{} is not configured", kind.label()) });
            continue;
        };
        match client.command(body).await {
            Ok(()) => {
                sqlx::query("UPDATE media_files SET upgrade_requested_at = ? WHERE id = ?").bind(now()).bind(id).execute(&state.pool).await?;
                out.push(SearchResult { id, title, success: true, message: format!("Search started in {}", kind.label()) });
            }
            Err(e) => out.push(SearchResult { id, title, success: false, message: format!("{e:#}") }),
        }
    }
    Ok(out)
}

/// Background: search for the worst candidates every `auto_interval_hours`.
pub fn spawn_background(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(e) = auto_run(&state).await {
                tracing::warn!(error = %format!("{e:#}"), "automatic upgrade search failed");
            }
        }
    });
}

async fn auto_run(state: &AppState) -> Result<()> {
    let s: UpgradeSettings = settings::load(&state.pool).await?;
    if !s.auto_enabled {
        return Ok(());
    }
    let last: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'upgrades_last_auto'").fetch_optional(&state.pool).await?;
    let due = last
        .and_then(|l| chrono::DateTime::parse_from_rfc3339(l.trim_matches('"')).ok())
        .is_none_or(|t| chrono::Utc::now() - t.with_timezone(&chrono::Utc) >= chrono::Duration::hours(s.auto_interval_hours.max(1)));
    if !due {
        return Ok(());
    }
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('upgrades_last_auto', ?)")
        .bind(serde_json::to_string(&now())?)
        .execute(&state.pool)
        .await?;
    let (_, files) = candidates(state, s.max_quality_score, false, s.auto_batch_size.clamp(1, 100)).await?;
    if files.is_empty() {
        return Ok(());
    }
    let ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let results = search(state, &ids).await?;
    let ok: Vec<&SearchResult> = results.iter().filter(|r| r.success).collect();
    tracing::info!(searched = ok.len(), "automatic upgrade search");
    let n: crate::notify::NotificationSettings = settings::load(&state.pool).await.unwrap_or_default();
    if n.on_upgrades && !ok.is_empty() {
        let lines: Vec<String> = ok.iter().map(|r| format!("• {}", r.title)).collect();
        crate::notify::send(
            state,
            "upgrades",
            &format!("Searching for better releases of {} file(s)", ok.len()),
            &lines.join("\n"),
            json!({ "fileIds": ids }),
        )
        .await;
    }
    Ok(())
}
