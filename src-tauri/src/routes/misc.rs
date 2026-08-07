use axum::response::sse::{Event, Sse};
use futures_util::StreamExt;
use std::convert::Infallible;
use crate::api_server::*;

pub async fn api_events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = get_sse_tx().subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(m) => Some(Ok(Event::default().event(m.event).data(m.data))),
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}
