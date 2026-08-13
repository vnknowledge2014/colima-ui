//! SSE (Server-Sent Events) broadcast infrastructure.
//!
//! Provides a pub/sub channel for pushing real-time updates to browser clients,
//! plus background watchers for Docker events and instance state changes.
//!
//! # This is a display channel, not a durable one
//!
//! The broadcast channel holds 64 messages. A client slower than the producer
//! misses whatever overflowed, and there is no replay — the loss is reported to
//! that client as a `stream-lagged` event so a chart can draw a gap instead of a
//! straight line through missing data.
//!
//! Anything that must not lose a sample therefore cannot read it here. Write
//! from the producer instead, and treat SSE as the view.
//!
//! # Counting who is listening
//!
//! [`subscriber_count`] answers "is anyone watching this topic right now", which
//! is what lets an expensive producer stay stopped until someone opens the
//! screen that needs it. The count is maintained by [`SubscriptionGuard`], whose
//! `Drop` runs when the HTTP stream ends — including the cases an explicit
//! unsubscribe endpoint would miss: a closed tab, a reload, a killed browser, a
//! dropped network.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use tokio::sync::broadcast;

// ===== Broadcast Channel =====

#[derive(Clone, Debug)]
pub struct SseMessage {
    pub event: String,
    pub data: String,
}

static SSE_TX: OnceLock<broadcast::Sender<SseMessage>> = OnceLock::new();

/// Live subscriber count per topic. Only clients that named a topic are counted
/// — see [`subscribe_topics`].
static SUBSCRIBERS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn get_sse_tx() -> broadcast::Sender<SseMessage> {
    SSE_TX
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(64);
            tx
        })
        .clone()
}

/// Keeps a subscription counted for as long as it is held.
///
/// The count is decremented on `Drop`, which is the whole point: it is tied to
/// the lifetime of the stream rather than to a client remembering to say
/// goodbye. A client that crashes mid-request still stops being counted.
pub struct SubscriptionGuard {
    topics: Vec<String>,
}

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        // A poisoned lock would mean some other thread panicked while holding
        // it. Leaking a count is better than panicking in a destructor.
        let Ok(mut subs) = SUBSCRIBERS.lock() else {
            return;
        };
        for topic in &self.topics {
            if let Some(count) = subs.get_mut(topic) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    subs.remove(topic);
                }
            }
        }
    }
}

/// Subscribe, registering interest in zero or more named topics.
///
/// Passing no topics is the backwards-compatible case: the caller receives every
/// event, and is counted as watching nothing in particular. That asymmetry is
/// deliberate — every client that existed before topics were introduced receives
/// everything, and treating them all as watchers of every topic would mean an
/// expensive producer could never stop.
pub fn subscribe_topics(topics: &[String]) -> (broadcast::Receiver<SseMessage>, SubscriptionGuard) {
    // The guard carries only what was actually counted. If the lock is poisoned
    // the increment does not happen, and a guard that still listed these topics
    // would decrement counts it never raised — driving a live topic to zero and
    // stopping a producer while someone is watching, which is precisely the
    // failure this registry exists to prevent.
    let counted = match SUBSCRIBERS.lock() {
        Ok(mut subs) => {
            for topic in topics {
                *subs.entry(topic.clone()).or_insert(0) += 1;
            }
            topics.to_vec()
        }
        Err(_) => Vec::new(),
    };

    (
        get_sse_tx().subscribe(),
        SubscriptionGuard { topics: counted },
    )
}

/// How many clients are currently watching `topic`.
///
/// Deliberately **not** `Sender::receiver_count`, which counts every SSE client
/// in the process — docker state, instances, Kubernetes — and so can never
/// answer a question about one topic.
pub fn subscriber_count(topic: &str) -> usize {
    SUBSCRIBERS
        .lock()
        .ok()
        .and_then(|subs| subs.get(topic).copied())
        .unwrap_or(0)
}

/// Publish an event to all connected SSE browser clients
pub fn publish_sse_event(event_type: &str, data: &serde_json::Value) {
    let tx = get_sse_tx();
    let _ = tx.send(SseMessage {
        event: event_type.to_string(),
        data: data.to_string(),
    });
}

// ===== Background Watchers =====

/// Push Docker state to SSE clients whenever it changes.
///
/// # It no longer opens its own event stream
///
/// It used to, which cost two things. Docker was asked to duplicate the same
/// event stream for two consumers in one process; and this copy had no reconnect
/// — a single stream error `break`s the loop, and the watcher was gone until the
/// app restarted. Since a stopped colima ends the stream, *"restart the VM"* left
/// browser-mode clients silently frozen with stale data.
///
/// Now it consumes [`crate::docker_events`], whose producer is the Tauri watcher
/// — the one that already owns the connection, the liveness ping and the
/// reconnect loop. Recovery comes for free, and there is one event stream in the
/// process rather than two.
///
/// The 500 ms trailing-edge debounce stays here, on the consumer side. That is
/// the point of the split: this watcher wants settled state and smooths the
/// burst, while consumers that need to count events read them unaggregated.
pub async fn sse_docker_watcher() {
    use tokio::sync::broadcast::error::RecvError;

    let mut events = crate::docker_events::subscribe();
    let debounce = std::time::Duration::from_millis(500);

    // Initial push, so a client connecting before anything happens still sees
    // the current state instead of an empty screen.
    if let Some(data) = fetch_current_docker_state().await {
        publish_sse_event("docker-state-updated", &data);
    }

    loop {
        match events.recv().await {
            Ok(_) => {}
            // Fell behind a burst. The refresh below reads current state anyway,
            // so the skipped events cost nothing here — this consumer never
            // needed them individually.
            Err(RecvError::Lagged(_)) => {}
            // The producer is a `OnceLock` static that lives for the process, so
            // this is unreachable in practice; treat it as "stop" rather than
            // spinning on a closed channel.
            Err(RecvError::Closed) => return,
        }

        // Trailing edge: keep swallowing events until 500 ms of quiet, then read
        // the settled state once.
        //
        // Matching the inner result rather than `is_ok()`: a closed channel
        // returns instantly and `Ok(Err(Closed))` *is* `is_ok()`, so the loop
        // would spin at full CPU. Unreachable while the producer is a static
        // that outlives the process, which is exactly why it would be missed.
        // Continues on a delivered event or a lag; stops on `Closed` (producer
        // gone) and on the timeout (the window elapsed).
        while let Ok(Ok(_)) | Ok(Err(RecvError::Lagged(_))) =
            tokio::time::timeout(debounce, events.recv()).await
        {}

        if let Some(data) = fetch_current_docker_state().await {
            publish_sse_event("docker-state-updated", &data);
        }
    }
}

/// Connect and read current state.
///
/// Uses `docker_state::connect_bollard` rather than a private copy. The private
/// copy this replaced only knew how to find a colima socket, so on a machine
/// running Docker Desktop the Tauri side worked and browser mode silently
/// showed nothing — two connection helpers that disagree is a bug waiting to be
/// written, so there is one.
///
/// A fresh client per refresh, rather than one held for the life of the watcher:
/// the socket path changes when colima is recreated, so a cached client outlives
/// the socket it points at. This runs at most once per debounce window.
async fn fetch_current_docker_state() -> Option<serde_json::Value> {
    let docker = crate::docker_state::connect_bollard()?;
    fetch_docker_state(&docker).await
}

/// Fetch current Docker containers + images and return as JSON.
async fn fetch_docker_state(docker: &bollard::Docker) -> Option<serde_json::Value> {
    use bollard::container::ListContainersOptions;
    use bollard::image::ListImagesOptions;

    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    let images = docker
        .list_images(Some(ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    // Reuse the single mapper rather than keeping a second copy here. The
    // two had already drifted — this one emitted a lowercase "id" — and a
    // divergence like that means a feature works in one mode and silently
    // not the other.
    let mapped_containers = crate::docker_state::map_containers(&containers);

    let mut mapped_images = Vec::new();
    for i in images {
        let tags = i.repo_tags.clone();
        let (repo, tag) = if !tags.is_empty() && tags[0] != "<none>:<none>" {
            let parts: Vec<&str> = tags[0].split(':').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (tags[0].clone(), "latest".to_string())
            }
        } else {
            ("<none>".to_string(), "<none>".to_string())
        };

        mapped_images.push(serde_json::json!({
            "id": i.id.replace("sha256:", ""),
            "Repository": repo,
            "Tag": tag,
            "Size": i.size.to_string(),
            "CreatedAt": i.created.to_string(),
        }));
    }

    Some(serde_json::json!({
        "containers": mapped_containers,
        "images": mapped_images
    }))
}

/// Periodically publish instance state to SSE clients
pub async fn sse_instance_publisher() {
    let mut last_json = String::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let instances = crate::instance_reader::list_instances_fast();
        let data = serde_json::json!({ "instances": instances });
        let json = data.to_string();
        // Only publish if state actually changed (avoid noise)
        if json != last_json {
            publish_sse_event("instances-update", &data);
            last_json = json;
        }
    }
}

#[cfg(test)]
mod subscriber_tests {
    use super::*;

    /// Topic names are per-test so the shared registry cannot make two tests
    /// interfere when the suite runs in parallel.
    fn topic(tag: &str) -> Vec<String> {
        vec![format!("test.topic.{tag}")]
    }

    #[test]
    fn a_subscription_is_counted_while_it_is_held() {
        let t = topic("held");
        assert_eq!(subscriber_count(&t[0]), 0);

        let (_rx, guard) = subscribe_topics(&t);
        assert_eq!(subscriber_count(&t[0]), 1);

        drop(guard);
        assert_eq!(subscriber_count(&t[0]), 0);
    }

    /// The count has to survive several watchers and return to zero only when
    /// the last one goes — otherwise a producer stops while someone is looking.
    #[test]
    fn the_count_tracks_several_watchers() {
        let t = topic("several");
        let (_a_rx, a) = subscribe_topics(&t);
        let (_b_rx, b) = subscribe_topics(&t);
        let (_c_rx, c) = subscribe_topics(&t);
        assert_eq!(subscriber_count(&t[0]), 3);

        drop(b);
        assert_eq!(subscriber_count(&t[0]), 2);

        drop(a);
        drop(c);
        assert_eq!(subscriber_count(&t[0]), 0);
    }

    /// A client that names no topic still receives everything, but must not be
    /// counted as watching anything — otherwise every pre-existing SSE client
    /// would pin every future producer permanently on.
    #[test]
    fn a_topicless_subscriber_is_not_counted() {
        let t = topic("topicless");
        let (_rx, _guard) = subscribe_topics(&[]);
        assert_eq!(subscriber_count(&t[0]), 0);
    }

    #[test]
    fn one_client_can_watch_several_topics() {
        let a = "test.topic.multi.a".to_string();
        let b = "test.topic.multi.b".to_string();
        let (_rx, guard) = subscribe_topics(&[a.clone(), b.clone()]);
        assert_eq!(subscriber_count(&a), 1);
        assert_eq!(subscriber_count(&b), 1);

        drop(guard);
        assert_eq!(subscriber_count(&a), 0);
        assert_eq!(subscriber_count(&b), 0);
    }

    /// Dropping the guard is what a disconnect looks like from here — there is
    /// no cooperation required from the client, which is the reason for the
    /// guard rather than an unsubscribe endpoint.
    #[test]
    fn an_abandoned_receiver_still_stops_being_counted() {
        let t = topic("abandoned");
        {
            let (rx, _guard) = subscribe_topics(&t);
            drop(rx); // client vanished without a word
            assert_eq!(subscriber_count(&t[0]), 1, "guard still held");
        }
        assert_eq!(subscriber_count(&t[0]), 0, "guard dropped with the stream");
    }
}
