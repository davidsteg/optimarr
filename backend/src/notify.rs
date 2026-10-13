//! Notifications to Discord, ntfy or any webhook.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::now;
use crate::settings::{self, SettingsDoc};
use crate::state::AppState;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Target {
    pub name: String,
    /// discord | ntfy | webhook
    pub kind: String,
    pub enabled: bool,
    /// Discord webhook URL, full ntfy topic URL (https://ntfy.sh/my-topic), or webhook URL.
    pub url: String,
    /// Optional bearer token (ntfy access token, webhook secret).
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationSettings {
    pub targets: Vec<Target>,
    /// New files that are broken or score poorly.
    pub on_new_problems: bool,
    pub on_fix_finished: bool,
    pub on_upgrades: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self { targets: Vec::new(), on_new_problems: true, on_fix_finished: true, on_upgrades: true }
    }
}

impl SettingsDoc for NotificationSettings {
    const KEY: &'static str = "notifications";
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub target: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn human_size(bytes: i64) -> String {
    let b = bytes as f64;
    if b >= 1e12 {
        format!("{:.1} TB", b / 1e12)
    } else if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else {
        format!("{:.0} MB", b / 1e6)
    }
}

/// Split `https://ntfy.sh/topic` into the server URL and the topic.
fn ntfy_parts(url: &str) -> Result<(String, String)> {
    let url = url.trim().trim_end_matches('/');
    let (base, topic) = url.rsplit_once('/').context("ntfy URL must end with the topic, e.g. https://ntfy.sh/optimarr")?;
    if topic.is_empty() || !base.contains("://") || base.ends_with(':') || base.ends_with('/') {
        bail!("ntfy URL must end with the topic, e.g. https://ntfy.sh/optimarr");
    }
    Ok((base.to_string(), topic.to_string()))
}

async fn deliver(http: &reqwest::Client, t: &Target, event: &str, title: &str, message: &str, data: &Value) -> Result<()> {
    let auth = |r: reqwest::RequestBuilder| if t.token.is_empty() { r } else { r.bearer_auth(&t.token) };
    let req = match t.kind.as_str() {
        "discord" => {
            let mut description: String = message.chars().take(3900).collect();
            if description.len() < message.len() {
                description.push('…');
            }
            http.post(&t.url).json(&json!({
                "username": "Optimarr",
                "embeds": [{ "title": title, "description": description, "color": 0x10b981 }]
            }))
        }
        "ntfy" => {
            let (base, topic) = ntfy_parts(&t.url)?;
            auth(http.post(base)).json(&json!({ "topic": topic, "title": title, "message": message, "tags": ["clapper"] }))
        }
        "webhook" => auth(http.post(&t.url)).json(&json!({ "event": event, "title": title, "message": message, "data": data })),
        other => bail!("unknown notification type {other}"),
    };
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body: String = resp.text().await.unwrap_or_default().chars().take(200).collect();
        bail!("{status} {body}");
    }
    Ok(())
}

/// Send to every enabled target and record the attempt. Never fails; errors are logged.
pub async fn send(state: &AppState, event: &str, title: &str, message: &str, data: Value) -> Vec<Delivery> {
    let s: NotificationSettings = settings::load(&state.pool).await.unwrap_or_default();
    send_to(state, &s.targets.iter().filter(|t| t.enabled).cloned().collect::<Vec<_>>(), event, title, message, data).await
}

pub async fn send_to(state: &AppState, targets: &[Target], event: &str, title: &str, message: &str, data: Value) -> Vec<Delivery> {
    if targets.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for t in targets {
        let name = if t.name.is_empty() { t.kind.clone() } else { t.name.clone() };
        match deliver(&state.http, t, event, title, message, &data).await {
            Ok(()) => out.push(Delivery { target: name, ok: true, error: None }),
            Err(e) => {
                tracing::warn!(target = %name, error = %format!("{e:#}"), "notification failed");
                out.push(Delivery { target: name, ok: false, error: Some(format!("{e:#}")) });
            }
        }
    }
    let _ = sqlx::query("INSERT INTO notifications (event, title, message, sent_at, results) VALUES (?, ?, ?, ?, ?)")
        .bind(event)
        .bind(title)
        .bind(message)
        .bind(now())
        .bind(serde_json::to_string(&out).unwrap_or_default())
        .execute(&state.pool)
        .await;
    out
}

/// After a scan or watcher run: report files first seen since `since` that are broken or poor.
pub async fn new_problems(state: &AppState, since: &str) {
    let s: NotificationSettings = settings::load(&state.pool).await.unwrap_or_default();
    if !s.on_new_problems || !s.targets.iter().any(|t| t.enabled) {
        return;
    }
    let rows: Vec<(i64, String, String, String, String, Option<String>)> = match sqlx::query_as(
        "SELECT id, COALESCE(radarr_movie_title, sonarr_series_title, file_name), status, compat_verdict, quality_verdict, status_reason
         FROM media_files WHERE first_seen_at >= ? AND status != 'missing'
           AND (status = 'broken' OR compat_verdict = 'poor' OR quality_verdict = 'poor')
         ORDER BY status = 'broken' DESC, compat_score + quality_score LIMIT 500",
    )
    .bind(since)
    .fetch_all(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "cannot query new problems");
            return;
        }
    };
    if rows.is_empty() {
        return;
    }
    let broken = rows.iter().filter(|r| r.2 == "broken").count();
    let title = match (broken, rows.len() - broken) {
        (0, n) => format!("{n} new file(s) with problems"),
        (b, 0) => format!("{b} new broken file(s)"),
        (b, n) => format!("{b} broken and {n} problem file(s) added"),
    };
    let mut lines: Vec<String> = rows
        .iter()
        .take(15)
        .map(|(_, t, status, cv, qv, reason)| {
            if status == "broken" {
                format!("• {t}: broken ({})", reason.as_deref().unwrap_or("unreadable"))
            } else {
                format!("• {t}: compatibility {cv}, quality {qv}")
            }
        })
        .collect();
    if rows.len() > 15 {
        lines.push(format!("…and {} more", rows.len() - 15));
    }
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    send(state, "new_problems", &title, &lines.join("\n"), json!({ "fileIds": ids })).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_ntfy_urls() {
        assert_eq!(ntfy_parts("https://ntfy.sh/optimarr").unwrap(), ("https://ntfy.sh".into(), "optimarr".into()));
        assert_eq!(ntfy_parts("http://ntfy.local:8080/sub/alerts/").unwrap(), ("http://ntfy.local:8080/sub".into(), "alerts".into()));
        assert!(ntfy_parts("https://ntfy.sh/").is_err());
        assert!(ntfy_parts("https://ntfy.sh").is_err());
    }
}
