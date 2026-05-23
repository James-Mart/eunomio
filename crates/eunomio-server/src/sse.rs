// SPDX-License-Identifier: Apache-2.0

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

const KEEP_ALIVE_SECS: u64 = 15;

/// Wraps a `broadcast::Receiver` plus an optional initial snapshot into an
/// `Sse` stream that emits each `T` as a JSON `data:` event with a 15-second
/// keep-alive comment. Used by both the per-session coordinator stream and
/// the singleton tunnel-status stream.
pub fn json_broadcast_stream<T>(
    rx: broadcast::Receiver<T>,
    initial: Option<T>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
    T: Serialize + Clone + Send + 'static,
{
    let initial_stream = futures::stream::iter(
        initial
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
            .map(|data| Ok(Event::default().data(data))),
    );
    let updates = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(value) => match serde_json::to_string(&value) {
                Ok(data) => Some(Ok(Event::default().data(data))),
                Err(e) => {
                    tracing::error!(error = %e, "failed to serialise SSE event");
                    None
                }
            },
            Err(_) => None,
        }
    });
    Sse::new(initial_stream.chain(updates))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(KEEP_ALIVE_SECS)))
}
