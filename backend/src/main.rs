mod api;
mod auth;
mod background;
mod db;
mod integrations;
mod jobs;
mod legacy;
mod media;
mod repo;
mod scanner;
mod settings;
mod state;
mod web;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub const DB_FILE: &str = "optimarr2.db";

#[derive(Parser, Debug)]
#[command(version, about = "Optimarr: media library compatibility and quality analyzer")]
struct Args {
    /// Address to listen on.
    #[arg(long, env = "OPTIMARR_BIND", default_value = "0.0.0.0:8080")]
    bind: SocketAddr,
    /// Directory for the database.
    #[arg(long, env = "OPTIMARR_DATA_DIR", default_value = "data")]
    data_dir: PathBuf,
    /// Directory checked for an Optimarr 1.x appsettings.json to import.
    #[arg(long, env = "OPTIMARR_CONFIG_DIR", default_value = "config")]
    config_dir: PathBuf,
    /// Directory for log files (daily rotation). Empty disables file logging.
    #[arg(long, env = "OPTIMARR_LOG_DIR", default_value = "logs")]
    log_dir: String,
    /// Fixed API key (overrides the generated one).
    #[arg(long, env = "OPTIMARR_API_KEY")]
    api_key: Option<String>,
    /// Extra allowed CORS origins, comma separated.
    #[arg(long, env = "OPTIMARR_CORS_ORIGINS", default_value = "")]
    cors_origins: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let filter = EnvFilter::try_from_env("OPTIMARR_LOG").unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=warn"));
    let _guard = if args.log_dir.is_empty() {
        tracing_subscriber::registry().with(filter).with(fmt::layer()).init();
        None
    } else {
        std::fs::create_dir_all(&args.log_dir).ok();
        let appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("optimarr")
            .filename_suffix("log")
            .max_log_files(14)
            .build(&args.log_dir)?;
        let (nb, guard) = tracing_appender::non_blocking(appender);
        tracing_subscriber::registry().with(filter).with(fmt::layer()).with(fmt::layer().with_ansi(false).with_writer(nb)).init();
        Some(guard)
    };

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting Optimarr");
    std::fs::create_dir_all(&args.data_dir).with_context(|| format!("cannot create data dir {}", args.data_dir.display()))?;
    let pool = db::open(&args.data_dir.join(DB_FILE)).await.context("cannot open database")?;
    legacy::import_if_needed(&pool, &args.data_dir, &args.config_dir).await?;
    auth::init(&pool, args.api_key.clone()).await?;

    let state = state::AppState::new(
        pool,
        state::Config { data_dir: args.data_dir.clone(), api_key: args.api_key.clone().unwrap_or_default(), version: env!("CARGO_PKG_VERSION") },
    )
    .await?;
    background::spawn_all(state.clone());

    let origins: Vec<_> = args.cors_origins.split(',').map(str::trim).filter(|o| !o.is_empty()).filter_map(|o| o.parse().ok()).collect();
    let app = api::router()
        .merge(web::router())
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::middleware))
        .layer(CorsLayer::new().allow_origin(origins).allow_methods(tower_http::cors::Any).allow_headers(tower_http::cors::Any))
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(SetResponseHeaderLayer::overriding(axum::http::header::X_CONTENT_TYPE_OPTIONS, axum::http::HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(axum::http::header::X_FRAME_OPTIONS, axum::http::HeaderValue::from_static("SAMEORIGIN")))
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(args.bind).await.with_context(|| format!("cannot bind {}", args.bind))?;
    tracing::info!("listening on http://{}", args.bind);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            let term = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal").recv().await;
            };
            #[cfg(not(unix))]
            let term = std::future::pending::<()>();
            tokio::select! { _ = ctrl_c => {}, _ = term => {} }
            tracing::info!("shutting down");
        })
        .await?;
    Ok(())
}
