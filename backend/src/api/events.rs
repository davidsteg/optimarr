use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::jobs::Event;
use crate::state::AppState;

/// Server-sent events: job progress and change notifications.
pub async fn stream(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let initial: Vec<Event> = state.jobs.running().into_iter().map(|job| Event::Job { job }).collect();
    let live = BroadcastStream::new(state.jobs.subscribe()).filter_map(|e| e.ok());
    let stream = tokio_stream::iter(initial).chain(live).map(|e| Ok(SseEvent::default().json_data(e).unwrap_or_else(|_| SseEvent::default())));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
