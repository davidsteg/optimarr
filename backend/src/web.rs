//! Serves the embedded single-page app.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rust_embed::RustEmbed;

use crate::state::AppState;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../frontend/build"]
struct Assets;

pub fn router() -> Router<AppState> {
    Router::new().fallback(serve)
}

async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({"error": "Not found"}))).into_response();
    }
    if let Some(r) = asset(path) {
        return r;
    }
    // SPA fallback.
    asset("index.html").unwrap_or_else(|| {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html")],
            "<h1>Optimarr</h1><p>The web UI was not built. Run <code>npm run build</code> in <code>frontend/</code>. The API is available under /api.</p>",
        )
            .into_response()
    })
}

fn asset(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Hashed build assets can be cached forever; everything else must revalidate.
    let cache = if path.contains("/immutable/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    Some(
        Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache)
            .header(header::ETAG, format!("\"{}\"", hex(&file.metadata.sha256_hash()[..8])))
            .body(Body::from(file.data.into_owned()))
            .unwrap(),
    )
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
