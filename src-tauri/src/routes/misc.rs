use crate::api_server::*;
use axum::extract::RawQuery;
use axum::response::sse::{Event, Sse};
use futures_util::StreamExt;
use std::convert::Infallible;

/// Caps on what one client may register.
///
/// The topic map is process-global and its keys come straight from the request,
/// so without a bound an authenticated caller could hold arbitrarily many keys
/// for the life of a connection, and every published event would be matched
/// against all of them.
const MAX_TOPICS: usize = 16;
const MAX_TOPIC_LEN: usize = 64;

/// Topics named by `?topics=a,b` — repeatable, so `?topics=a&topics=b` works too.
///
/// Parsed straight from the raw query rather than through `Query<T>` on purpose.
/// A typed extractor rejects shapes it cannot deserialize, and `?topics=a&topics=b`
/// became a **400** on an endpoint that previously accepted any query at all. A
/// browser's `EventSource` treats that as a hard failure, so narrowing the
/// accepted input of a reconnecting stream is not a trade worth making for
/// tidier code.
///
/// Values are not percent-decoded, matching `auth::token_from_query`, which
/// parses the same query string the same way. Topic names are dotted
/// identifiers; anything needing an escape is not one.
fn parse_topics(raw_query: Option<&str>) -> Vec<String> {
    let Some(query) = raw_query else {
        return Vec::new();
    };

    let mut topics: Vec<String> = Vec::new();
    for value in query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .filter(|(k, _)| *k == "topics")
        .flat_map(|(_, v)| v.split(','))
    {
        let topic = value.trim();
        // Empty segments come from `?topics=` and `a,,b`; a subscription to the
        // empty string would be counted forever and never match an event.
        if topic.is_empty() || topic.len() > MAX_TOPIC_LEN {
            continue;
        }
        // One client asking for the same topic twice is one watcher, not two —
        // an inflated count keeps a producer running with nobody looking.
        if topics.iter().any(|t| t == topic) {
            continue;
        }
        topics.push(topic.to_string());
        if topics.len() == MAX_TOPICS {
            break;
        }
    }
    topics
}

/// Event name a client receives when the channel dropped messages it never saw.
pub const LAGGED_EVENT: &str = "stream-lagged";

/// Decide what one item from the broadcast stream becomes on the wire.
///
/// Returned as `(event, data)` rather than an `axum` `Event` so it can be
/// asserted on: `Event` exposes no accessors, which would have left the lag path
/// — the one this exists to fix — untestable.
fn classify(
    msg: Result<SseMessage, tokio_stream::wrappers::errors::BroadcastStreamRecvError>,
    topics: &[String],
) -> Option<(String, String)> {
    use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

    match msg {
        Ok(m) => {
            // No topics named means "everything", for backwards compatibility.
            if topics.is_empty() || topics.iter().any(|t| t == &m.event) {
                Some((m.event, m.data))
            } else {
                None
            }
        }

        // The client fell behind and the channel dropped `n` messages.
        //
        // This used to map to `None`, which made the loss invisible: a chart drew
        // a straight line across the hole, and nobody could tell the difference
        // between "nothing happened" and "we lost the data". Report it instead,
        // and let the UI render a gap.
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            Some((LAGGED_EVENT.to_string(), format!(r#"{{"dropped":{n}}}"#)))
        }
    }
}

pub async fn api_events(
    RawQuery(query): RawQuery,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let topics = parse_topics(query.as_deref());
    let (rx, guard) = subscribe_topics(&topics);

    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |msg| {
        // The guard is owned by this closure, so the subscription stays counted
        // until the stream is dropped — which is when the client goes away, by
        // any route including one that never says so.
        let _keep_subscription_counted = &guard;

        let out = classify(msg, &topics)
            .map(|(event, data)| Ok(Event::default().event(event).data(data)));
        async move { out }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_parse_into_a_clean_list() {
        assert_eq!(
            parse_topics(Some("topics=metrics.sample")),
            vec!["metrics.sample"]
        );
        assert_eq!(parse_topics(Some("topics=a,b")), vec!["a", "b"]);
        assert_eq!(parse_topics(Some("token=abc&topics=a")), vec!["a"]);
    }

    /// `?topics=a&topics=b` must work. A typed extractor rejected it with a 400
    /// on an endpoint that previously accepted any query, and a 400 to an
    /// `EventSource` is a hard failure rather than a retry.
    #[test]
    fn a_repeated_parameter_is_accepted_rather_than_rejected() {
        assert_eq!(parse_topics(Some("topics=a&topics=b")), vec!["a", "b"]);
        assert_eq!(
            parse_topics(Some("token=x&topics=a&follow=1&topics=b")),
            vec!["a", "b"]
        );
    }

    /// An absent or blank parameter must mean "no topic subscription", not a
    /// subscription to the empty string — which would be counted forever and
    /// never match an event.
    #[test]
    fn a_blank_topics_parameter_registers_nothing() {
        assert!(parse_topics(None).is_empty());
        assert!(parse_topics(Some("")).is_empty());
        assert!(parse_topics(Some("topics=")).is_empty());
        assert!(parse_topics(Some("topics=,,")).is_empty());
        assert!(parse_topics(Some("token=abc")).is_empty());
        assert_eq!(parse_topics(Some("topics=a,,b")), vec!["a", "b"]);
    }

    /// One client asking twice is one watcher. An inflated count keeps a
    /// producer running with nobody looking.
    #[test]
    fn the_same_topic_twice_counts_once() {
        assert_eq!(parse_topics(Some("topics=a,a")), vec!["a"]);
        assert_eq!(parse_topics(Some("topics=a&topics=a")), vec!["a"]);
    }

    /// The keys land in a process-global map and are held for the life of the
    /// connection, so one request must not be able to register unbounded work.
    #[test]
    fn a_client_cannot_register_unbounded_topics() {
        let many: Vec<String> = (0..500).map(|i| format!("t{i}")).collect();
        let query = format!("topics={}", many.join(","));
        assert_eq!(parse_topics(Some(&query)).len(), MAX_TOPICS);

        let long = "x".repeat(MAX_TOPIC_LEN + 1);
        assert!(parse_topics(Some(&format!("topics={long}"))).is_empty());
    }

    fn msg(event: &str) -> Result<SseMessage, tokio_stream::wrappers::errors::BroadcastStreamRecvError>
    {
        Ok(SseMessage {
            event: event.to_string(),
            data: "{}".to_string(),
        })
    }

    /// The whole reason this phase exists: a dropped batch must reach the client
    /// as a fact, not as silence.
    #[test]
    fn dropped_messages_are_reported_rather_than_swallowed() {
        use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

        let (event, data) = classify(Err(BroadcastStreamRecvError::Lagged(17)), &[])
            .expect("a lag must produce an event");

        assert_eq!(event, LAGGED_EVENT);
        assert!(data.contains("17"), "the count must survive: {data}");
    }

    /// A lag is reported even to a client that filtered down to one topic — it
    /// has no way to know which topic the lost messages belonged to, so hiding
    /// the gap would be worse than mentioning one that was not theirs.
    #[test]
    fn a_lag_is_reported_even_when_filtering() {
        use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

        let topics = vec!["metrics.sample".to_string()];
        let out = classify(Err(BroadcastStreamRecvError::Lagged(3)), &topics);
        assert_eq!(out.map(|(e, _)| e), Some(LAGGED_EVENT.to_string()));
    }

    #[test]
    fn a_client_that_named_no_topic_receives_everything() {
        assert!(classify(msg("docker-state-updated"), &[]).is_some());
        assert!(classify(msg("instances-update"), &[]).is_some());
    }

    #[test]
    fn naming_a_topic_filters_out_the_rest() {
        let topics = vec!["metrics.sample".to_string()];
        assert!(classify(msg("metrics.sample"), &topics).is_some());
        assert!(classify(msg("docker-state-updated"), &topics).is_none());
    }
}
