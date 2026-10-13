//! Sonarr and Radarr (API v3) client, library sync, file matching and redownloads.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::now;
use crate::jobs::{Event, JobHandle};
use crate::settings::{self, map_path, unmap_path, RadarrSettings, ServarrSettings, SonarrSettings};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Sonarr,
    Radarr,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Sonarr => "sonarr",
            Kind::Radarr => "radarr",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Sonarr => "Sonarr",
            Kind::Radarr => "Radarr",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "sonarr" => Some(Kind::Sonarr),
            "radarr" => Some(Kind::Radarr),
            _ => None,
        }
    }
}

pub async fn load_settings(state: &AppState, kind: Kind) -> Result<ServarrSettings> {
    Ok(match kind {
        Kind::Sonarr => settings::load::<SonarrSettings>(&state.pool).await?.0,
        Kind::Radarr => settings::load::<RadarrSettings>(&state.pool).await?.0,
    })
}

pub async fn save_settings(state: &AppState, kind: Kind, s: ServarrSettings) -> Result<()> {
    match kind {
        Kind::Sonarr => settings::save(&state.pool, &SonarrSettings(s)).await,
        Kind::Radarr => settings::save(&state.pool, &RadarrSettings(s)).await,
    }
}

pub struct Client {
    http: reqwest::Client,
    pub settings: ServarrSettings,
    pub kind: Kind,
}

impl Client {
    pub fn new(http: reqwest::Client, kind: Kind, settings: ServarrSettings) -> Self {
        Self { http, settings, kind }
    }

    pub async fn from_state(state: &AppState, kind: Kind) -> Result<Self> {
        let s = load_settings(state, kind).await?;
        if !s.enabled || s.base_url.trim().is_empty() || s.api_key.trim().is_empty() {
            bail!("{} is not configured", kind.label());
        }
        Ok(Self::new(state.http.clone(), kind, s))
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v3/{}", self.settings.base_url.trim_end_matches('/'), path.trim_start_matches('/'))
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut r = self.http.request(method, self.url(path)).header("X-Api-Key", self.settings.api_key.trim()).header("Accept", "application/json");
        if let Some(u) = self.settings.basic_auth_username.as_ref().filter(|u| !u.is_empty()) {
            r = r.basic_auth(u, self.settings.basic_auth_password.clone());
        }
        r
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let resp = self.req(reqwest::Method::GET, path).send().await.with_context(|| format!("{} unreachable", self.kind.label()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("{} returned {status} for {path}: {}", self.kind.label(), body.chars().take(200).collect::<String>());
        }
        resp.json().await.with_context(|| format!("unexpected response from {} {path}", self.kind.label()))
    }

    pub async fn status(&self) -> Result<String> {
        let v: Value = self.get("system/status").await?;
        Ok(v.get("version").and_then(Value::as_str).unwrap_or("unknown").to_string())
    }

    pub async fn root_folders(&self) -> Result<Vec<RootFolder>> {
        self.get("rootfolder").await
    }

    pub async fn delete_file(&self, file_id: i64) -> Result<()> {
        let path = match self.kind {
            Kind::Sonarr => format!("episodefile/{file_id}"),
            Kind::Radarr => format!("moviefile/{file_id}"),
        };
        let resp = self.req(reqwest::Method::DELETE, &path).send().await?;
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            bail!("{} refused to delete file {file_id}: {}", self.kind.label(), resp.status());
        }
        Ok(())
    }

    pub async fn command(&self, body: Value) -> Result<()> {
        let resp = self.req(reqwest::Method::POST, "command").json(&body).send().await?;
        if !resp.status().is_success() {
            bail!("{} command failed: {}", self.kind.label(), resp.status());
        }
        Ok(())
    }

    /// Every file Sonarr/Radarr knows about, keyed by the *local* path.
    pub async fn file_index(&self) -> Result<HashMap<String, Match>> {
        let mappings = &self.settings.path_mappings;
        let mut out = HashMap::new();
        match self.kind {
            Kind::Radarr => {
                let movies: Vec<RadarrMovie> = self.get("movie").await?;
                for m in movies {
                    if let Some(f) = m.movie_file.filter(|f| !f.path.is_empty()) {
                        out.insert(
                            norm_key(&map_path(mappings, &f.path)),
                            Match::Movie { movie_id: m.id, file_id: f.id, title: m.title.clone(), year: m.year },
                        );
                    }
                }
            }
            Kind::Sonarr => {
                let series: Vec<SonarrSeries> = self.get("series").await?;
                let results: Vec<Result<Vec<(String, Match)>>> = futures::stream::iter(series)
                    .map(|s| async move {
                        let files: Vec<SonarrEpisodeFile> = self.get(&format!("episodefile?seriesId={}", s.id)).await?;
                        if files.is_empty() {
                            return Ok(vec![]);
                        }
                        let episodes: Vec<SonarrEpisode> = self.get(&format!("episode?seriesId={}", s.id)).await?;
                        let mut by_file: HashMap<i64, Vec<&SonarrEpisode>> = HashMap::new();
                        for e in &episodes {
                            if let Some(fid) = e.episode_file_id.filter(|f| *f > 0) {
                                by_file.entry(fid).or_default().push(e);
                            }
                        }
                        Ok(files
                            .into_iter()
                            .filter(|f| !f.path.is_empty())
                            .map(|f| {
                                let eps = by_file.get(&f.id).cloned().unwrap_or_default();
                                let first = eps.iter().min_by_key(|e| (e.season_number, e.episode_number));
                                (
                                    norm_key(&map_path(mappings, &f.path)),
                                    Match::Episode {
                                        series_id: s.id,
                                        series_title: s.title.clone(),
                                        year: s.year,
                                        file_id: f.id,
                                        episode_ids: eps.iter().map(|e| e.id).collect(),
                                        season: first.map(|e| e.season_number).unwrap_or(f.season_number),
                                        episode: first.map(|e| e.episode_number),
                                    },
                                )
                            })
                            .collect())
                    })
                    .buffer_unordered(8)
                    .collect()
                    .await;
                for r in results {
                    out.extend(r?);
                }
            }
        }
        Ok(out)
    }
}

/// Case-insensitive, slash-normalised key for comparing paths across systems.
pub fn norm_key(p: &str) -> String {
    let decoded = urlencoding::decode(p).map(|c| c.into_owned()).unwrap_or_else(|_| p.to_string());
    decoded.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFolder {
    pub id: i64,
    pub path: String,
    #[serde(default)]
    pub accessible: bool,
    #[serde(default)]
    pub free_space: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RadarrMovie {
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<i64>,
    #[serde(default)]
    movie_file: Option<FileRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileRef {
    id: i64,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrSeries {
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SonarrEpisodeFile {
    id: i64,
    #[serde(default)]
    path: String,
    #[serde(default)]
    season_number: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SonarrEpisode {
    id: i64,
    #[serde(default)]
    season_number: i64,
    #[serde(default)]
    episode_number: i64,
    #[serde(default)]
    episode_file_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Match {
    #[serde(rename_all = "camelCase")]
    Episode { series_id: i64, series_title: String, year: Option<i64>, file_id: i64, episode_ids: Vec<i64>, season: i64, episode: Option<i64> },
    #[serde(rename_all = "camelCase")]
    Movie { movie_id: i64, file_id: i64, title: String, year: Option<i64> },
}

// ------------------------------------------------------------------ jobs

/// Create/refresh libraries from Sonarr/Radarr root folders.
pub async fn sync_root_folders(state: &AppState, kind: Kind) -> Result<usize> {
    let client = Client::from_state(state, kind).await?;
    let folders = client.root_folders().await?;
    let mut created = 0;
    for f in &folders {
        let local = map_path(&client.settings.path_mappings, &f.path);
        let local = local.trim_end_matches('/').to_string();
        let name = std::path::Path::new(&local).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| local.clone());
        let category = match kind {
            Kind::Sonarr => "series",
            Kind::Radarr => "movies",
        };
        let res = sqlx::query(
            "INSERT INTO libraries (name, path, category, servarr_kind, servarr_root_folder_id, servarr_root_folder_path, created_at, last_synced_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET servarr_kind = excluded.servarr_kind,
                servarr_root_folder_id = excluded.servarr_root_folder_id,
                servarr_root_folder_path = excluded.servarr_root_folder_path,
                last_synced_at = excluded.last_synced_at",
        )
        .bind(format!("{} ({})", name, kind.label()))
        .bind(&local)
        .bind(category)
        .bind(kind.as_str())
        .bind(f.id)
        .bind(&f.path)
        .bind(now())
        .bind(now())
        .execute(&state.pool)
        .await?;
        if res.rows_affected() > 0 {
            created += 1;
        }
    }
    state.jobs.emit(Event::LibrariesChanged);
    Ok(created)
}

/// Link analyzed files to Sonarr episodes / Radarr movies.
pub async fn start_match(state: &AppState) -> Result<i64> {
    if let Some(j) = state.jobs.find_running("match", None) {
        return Ok(j.id);
    }
    let job = state.jobs.start("match", "Match files with Sonarr/Radarr", None).await?;
    let id = job.id;
    let st = state.clone();
    tokio::spawn(async move {
        let res = run_match(&st, &job).await;
        st.jobs.finish(&job, res).await;
        st.jobs.emit(Event::MediaChanged);
    });
    Ok(id)
}

async fn run_match(state: &AppState, job: &JobHandle) -> Result<()> {
    let mut index: HashMap<String, (Kind, Match)> = HashMap::new();
    let mut used = vec![];
    for kind in [Kind::Radarr, Kind::Sonarr] {
        let Ok(client) = Client::from_state(state, kind).await else { continue };
        job.set_message(format!("Loading {} library", kind.label()));
        let files = client.file_index().await?;
        tracing::info!(files = files.len(), "{} index loaded", kind.label());
        index.extend(files.into_iter().map(|(k, m)| (k, (kind, m))));
        used.push(kind);
    }
    if used.is_empty() {
        bail!("Neither Sonarr nor Radarr is configured");
    }

    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, path FROM media_files").fetch_all(&state.pool).await?;
    job.set_total(rows.len() as i64);
    job.set_message("Matching files");
    let mut matched = 0;
    let mut tx = state.pool.begin().await?;
    for (i, (id, path)) in rows.iter().enumerate() {
        match index.get(&norm_key(path)) {
            Some((kind, m)) => {
                apply_match(&mut *tx, *id, *kind, m).await?;
                matched += 1;
            }
            None => {
                // Clear stale links from services that were checked.
                for k in &used {
                    sqlx::query(
                        "UPDATE media_files SET servarr_kind=NULL, sonarr_series_id=NULL, sonarr_series_title=NULL,
                         sonarr_episode_id=NULL, sonarr_episode_file_id=NULL, season_number=NULL, episode_number=NULL,
                         radarr_movie_id=NULL, radarr_movie_file_id=NULL, radarr_movie_title=NULL, year=NULL, matched_at=NULL
                         WHERE id = ? AND servarr_kind = ?",
                    )
                    .bind(id)
                    .bind(k.as_str())
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        job.inc_processed();
        if i % 1000 == 999 {
            tx.commit().await?;
            tx = state.pool.begin().await?;
        }
    }
    tx.commit().await?;
    job.set_message(format!("Matched {matched} of {} files", rows.len()));
    Ok(())
}

async fn apply_match<'e, E: sqlx::SqliteExecutor<'e>>(ex: E, id: i64, kind: Kind, m: &Match) -> Result<()> {
    match m {
        Match::Episode { series_id, series_title, year, file_id, episode_ids, season, episode } => {
            sqlx::query(
                "UPDATE media_files SET servarr_kind=?, sonarr_series_id=?, sonarr_series_title=?, sonarr_episode_id=?,
                 sonarr_episode_file_id=?, season_number=?, episode_number=?, year=?, radarr_movie_id=NULL,
                 radarr_movie_file_id=NULL, radarr_movie_title=NULL, matched_at=? WHERE id=?",
            )
            .bind(kind.as_str())
            .bind(series_id)
            .bind(series_title)
            .bind(episode_ids.first())
            .bind(file_id)
            .bind(season)
            .bind(episode)
            .bind(year)
            .bind(now())
            .bind(id)
            .execute(ex)
            .await?;
        }
        Match::Movie { movie_id, file_id, title, year } => {
            sqlx::query(
                "UPDATE media_files SET servarr_kind=?, radarr_movie_id=?, radarr_movie_file_id=?, radarr_movie_title=?, year=?,
                 sonarr_series_id=NULL, sonarr_series_title=NULL, sonarr_episode_id=NULL, sonarr_episode_file_id=NULL,
                 season_number=NULL, episode_number=NULL, matched_at=? WHERE id=?",
            )
            .bind(kind.as_str())
            .bind(movie_id)
            .bind(file_id)
            .bind(title)
            .bind(year)
            .bind(now())
            .bind(id)
            .execute(ex)
            .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedownloadResult {
    pub id: i64,
    pub success: bool,
    pub service: Option<String>,
    pub message: String,
}

/// Ask Sonarr/Radarr to replace files: delete the current file, then trigger a search.
/// Files are marked "processing" and re-analyzed automatically once the new file lands.
pub async fn redownload(state: &AppState, ids: &[i64]) -> Result<Vec<RedownloadResult>> {
    let mut clients: HashMap<Kind, Option<Client>> = HashMap::new();
    let mut indexes: HashMap<Kind, HashMap<String, Match>> = HashMap::new();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &id in ids {
        if !seen.insert(id) {
            continue;
        }
        let Some(file) = crate::repo::get_media(&state.pool, id).await? else {
            out.push(RedownloadResult { id, success: false, service: None, message: "File not found".into() });
            continue;
        };
        // Use the stored link, or look the file up (index cached per request).
        let mut target: Option<(Kind, Match)> = match file.servarr_kind.as_deref().and_then(Kind::parse) {
            Some(Kind::Sonarr) if file.sonarr_episode_file_id.is_some() => Some((
                Kind::Sonarr,
                Match::Episode {
                    series_id: file.sonarr_series_id.unwrap_or_default(),
                    series_title: file.sonarr_series_title.clone().unwrap_or_default(),
                    year: file.year,
                    file_id: file.sonarr_episode_file_id.unwrap(),
                    episode_ids: file.sonarr_episode_id.into_iter().collect(),
                    season: file.season_number.unwrap_or_default(),
                    episode: file.episode_number,
                },
            )),
            Some(Kind::Radarr) if file.radarr_movie_id.is_some() => Some((
                Kind::Radarr,
                Match::Movie {
                    movie_id: file.radarr_movie_id.unwrap(),
                    file_id: file.radarr_movie_file_id.unwrap_or_default(),
                    title: file.radarr_movie_title.clone().unwrap_or_default(),
                    year: file.year,
                },
            )),
            _ => None,
        };
        if target.is_none() {
            for kind in [Kind::Sonarr, Kind::Radarr] {
                if let std::collections::hash_map::Entry::Vacant(e) = clients.entry(kind) {
                    e.insert(Client::from_state(state, kind).await.ok());
                }
                let Some(Some(c)) = clients.get(&kind) else { continue };
                if let std::collections::hash_map::Entry::Vacant(e) = indexes.entry(kind) {
                    e.insert(c.file_index().await.unwrap_or_default());
                }
                if let Some(m) = indexes[&kind].get(&norm_key(&file.path)) {
                    target = Some((kind, m.clone()));
                    break;
                }
            }
        }
        let Some((kind, m)) = target else {
            out.push(RedownloadResult { id, success: false, service: None, message: "Not managed by Sonarr or Radarr (run a match first)".into() });
            continue;
        };
        if let std::collections::hash_map::Entry::Vacant(e) = clients.entry(kind) {
            e.insert(Client::from_state(state, kind).await.ok());
        }
        let Some(Some(client)) = clients.get(&kind) else {
            out.push(RedownloadResult {
                id,
                success: false,
                service: Some(kind.label().into()),
                message: format!("{} is not configured", kind.label()),
            });
            continue;
        };
        let res: Result<()> = async {
            match &m {
                Match::Episode { file_id, episode_ids, .. } => {
                    if episode_ids.is_empty() {
                        bail!("episode id unknown, re-run matching");
                    }
                    client.delete_file(*file_id).await?;
                    client.command(json!({"name": "EpisodeSearch", "episodeIds": episode_ids})).await
                }
                Match::Movie { movie_id, file_id, .. } => {
                    if *file_id > 0 {
                        client.delete_file(*file_id).await?;
                    }
                    client.command(json!({"name": "MoviesSearch", "movieIds": [movie_id]})).await
                }
            }
        }
        .await;
        match res {
            Ok(()) => {
                sqlx::query("UPDATE media_files SET status='processing', status_reason='Replacement requested', processing_since=? WHERE id=?")
                    .bind(now())
                    .bind(id)
                    .execute(&state.pool)
                    .await?;
                out.push(RedownloadResult {
                    id,
                    success: true,
                    service: Some(kind.label().into()),
                    message: format!("Search started in {}", kind.label()),
                });
            }
            Err(e) => out.push(RedownloadResult { id, success: false, service: Some(kind.label().into()), message: format!("{e:#}") }),
        }
    }
    state.jobs.emit(Event::MediaChanged);
    Ok(out)
}

/// Remote path as Sonarr/Radarr would see a local path (for display).
pub fn remote_path(s: &ServarrSettings, local: &str) -> String {
    unmap_path(&s.path_mappings, local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_keys() {
        assert_eq!(norm_key("D:\\Media\\TV\\Show%20A\\"), "d:/media/tv/show a");
        assert_eq!(norm_key("/tv/Show/S01E01.mkv"), "/tv/show/s01e01.mkv");
    }
}
