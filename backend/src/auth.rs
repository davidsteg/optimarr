//! API key authentication for /api and /mcp.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AuthSettings {
    pub api_key: String,
    /// "external": key required except from LAN/loopback; "always"; "never".
    pub mode: String,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self { api_key: String::new(), mode: "external".into() }
    }
}

impl crate::settings::SettingsDoc for AuthSettings {
    const KEY: &'static str = "auth";
}

pub fn generate_key() -> String {
    let mut rng = rand::rng();
    (0..32).map(|_| format!("{:x}", rng.random_range(0..16u8))).collect()
}

pub async fn store_key(pool: &SqlitePool, key: &str) -> anyhow::Result<()> {
    let mut s: AuthSettings = crate::settings::load(pool).await?;
    s.api_key = key.to_string();
    crate::settings::save(pool, &s).await
}

/// Ensure an API key exists; returns the effective settings (env var overrides the stored key).
pub async fn init(pool: &SqlitePool, env_key: Option<String>) -> anyhow::Result<AuthSettings> {
    let mut s: AuthSettings = crate::settings::load(pool).await?;
    if s.api_key.is_empty() {
        s.api_key = generate_key();
        crate::settings::save(pool, &s).await?;
        tracing::info!("generated API key (see Settings > General)");
    }
    if let Some(k) = env_key.filter(|k| !k.is_empty()) {
        s.api_key = k;
    }
    Ok(s)
}

fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().is_some_and(|v4| is_local(IpAddr::V4(v4)))
        }
    }
}

pub fn provided_key(req: &Request) -> Option<String> {
    let h = req.headers();
    for name in ["x-api-key", "x-optimarr-key"] {
        if let Some(v) = h.get(name).and_then(|v| v.to_str().ok()) {
            return Some(v.trim().to_string());
        }
    }
    if let Some(v) = h.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(t) = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")) {
            return Some(t.trim().to_string());
        }
    }
    // EventSource cannot send headers.
    req.uri().query().and_then(|q| {
        q.split('&').find_map(|kv| kv.strip_prefix("apikey=").map(|v| urlencoding::decode(v).map(|c| c.into_owned()).unwrap_or_default()))
    })
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub async fn current(state: &AppState) -> AuthSettings {
    let mut s: AuthSettings = crate::settings::load(&state.pool).await.unwrap_or_default();
    if !state.config.api_key.is_empty() {
        s.api_key = state.config.api_key.clone();
    }
    s
}

/// Is this request allowed without a key, given its source address?
pub async fn key_required(state: &AppState, peer: Option<SocketAddr>) -> bool {
    let s = current(state).await;
    match s.mode.as_str() {
        "never" => false,
        "always" => true,
        _ => !peer.map(|p| is_local(p.ip())).unwrap_or(false),
    }
}

pub async fn middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let protected = (path.starts_with("/api/") || path == "/mcp" || path.starts_with("/mcp/"))
        && !matches!(path, "/api/system/health" | "/api/system/auth");
    if !protected {
        return next.run(req).await;
    }
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let s = current(&state).await;
    let provided = provided_key(&req);
    let ok = match provided {
        Some(k) => constant_time_eq(&k, &s.api_key),
        None => !key_required(&state, peer).await,
    };
    if ok {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Unauthorized: send the API key in the X-Api-Key header"})))
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_addresses() {
        assert!(is_local("127.0.0.1".parse().unwrap()));
        assert!(is_local("192.168.1.5".parse().unwrap()));
        assert!(is_local("10.0.0.1".parse().unwrap()));
        assert!(is_local("::1".parse().unwrap()));
        assert!(is_local("::ffff:192.168.0.2".parse().unwrap()));
        assert!(!is_local("8.8.8.8".parse().unwrap()));
        assert!(!is_local("2001:4860::1".parse().unwrap()));
    }
}
