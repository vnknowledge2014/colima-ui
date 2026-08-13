//! Lossless container events.
//!
//! # Why this exists
//!
//! Both Docker watchers in this app consume the event stream as a *signal* —
//! "something changed, refetch everything" — and apply a 500 ms trailing-edge
//! debounce so the refetch sees settled state. That is right for a UI: a
//! `docker stop` fires kill → stop → die within ~200 ms and there is no reason
//! to redraw three times.
//!
//! It is wrong for anything that needs to *count* events. A container restarting
//! five times in ten seconds is one debounced refresh, so a crash-loop rule
//! reading that stream sees a single restart and never fires — while a
//! restart-on-unhealthy rule keeps firing forever. The information needed to
//! tell those apart was in the events, and the debounce threw it away.
//!
//! So this module carries every event through, unaggregated, with its payload
//! intact. Debouncing is a consumer's business: whoever wants smoothing does it
//! after the fork, where the raw data is still available to everyone else.
//!
//! # One stream, many consumers
//!
//! The producer is [`crate::docker_state::start_docker_watcher`], which already
//! owns the Docker connection, the ping-based liveness check and the reconnect
//! loop. It publishes here before applying its own debounce. Nothing else opens
//! an events stream — a second one would be a second thing to keep connected,
//! and would double the work Docker does for identical data.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::broadcast;

/// Room for a burst without dropping it.
///
/// Larger than the SSE channel's 64 because the point of this stream is that it
/// does not aggregate: a `compose up` of twenty services emits several events
/// each, and a consumer counting restarts must see all of them.
const CHANNEL_CAPACITY: usize = 512;

/// One container lifecycle event, as Docker reported it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ContainerEvent {
    /// Docker's own timestamp in seconds; 0 when the daemon omitted it.
    pub ts: i64,
    /// `start`, `die`, `kill`, `oom`, `health_status: unhealthy`, …
    pub action: String,
    pub container_id: String,
    /// Empty when Docker did not report a name, which happens for some actions.
    pub container_name: String,
    pub image: String,
    /// Present on `die`. `Some(0)` is a clean exit and `Some(137)` is a kill —
    /// the difference between "finished" and "was stopped", which a healing rule
    /// has to know before it decides to restart anything.
    pub exit_code: Option<i64>,
}

impl ContainerEvent {
    /// Was this container killed for exceeding its memory limit?
    ///
    /// Docker reports OOM two ways depending on version and runtime: a dedicated
    /// `oom` action, or a `die` whose actor carries `exitCode=137`. 137 is
    /// 128+SIGKILL, so it also covers an ordinary `docker kill` — which is why
    /// this is not the sole signal a caller should act on.
    pub fn is_oom(&self) -> bool {
        self.action == "oom"
    }
}

static EVENTS_TX: OnceLock<broadcast::Sender<ContainerEvent>> = OnceLock::new();

fn events_tx() -> &'static broadcast::Sender<ContainerEvent> {
    EVENTS_TX.get_or_init(|| broadcast::channel(CHANNEL_CAPACITY).0)
}

/// Receive every container event from now on.
///
/// A consumer that falls more than [`CHANNEL_CAPACITY`] behind receives
/// `RecvError::Lagged` and skips ahead. That is deliberate: a slow consumer must
/// not stall the producer or the other consumers, and a counting rule is better
/// off knowing it lost events than silently miscounting.
pub fn subscribe() -> broadcast::Receiver<ContainerEvent> {
    events_tx().subscribe()
}

/// Publish one event. Called only by the Docker watcher.
///
/// Sending fails when nobody is subscribed, which is the normal state — the
/// error is discarded rather than logged, because "no one is listening yet" is
/// not a problem worth a line per event.
pub fn publish(event: ContainerEvent) {
    let _ = events_tx().send(event);
}

/// Convert a raw bollard event, keeping only container **lifecycle** events.
///
/// Two filters, both load-bearing.
///
/// Type: image, network and volume events share the stream and have nothing to
/// do with container lifecycle, so a consumer counting restarts should not have
/// to recognise them.
///
/// Action: `exec_create`, `exec_start` and `exec_die` are container-type events
/// and are dropped anyway. Measured against a real daemon, healthcheck execs
/// alone were 18 of 33 container events in thirty seconds — a consumer would
/// spend most of its time discarding them. They also carry the full command line
/// in `action` (`exec_create: /bin/sh -c wget -qO- http://…`), and this struct
/// is `Serialize`: a healthcheck that authenticates would put its credentials
/// into anything that ships an event onward.
pub fn from_bollard(msg: &bollard::models::EventMessage) -> Option<ContainerEvent> {
    use bollard::models::EventMessageTypeEnum;

    if msg.typ != Some(EventMessageTypeEnum::CONTAINER) {
        return None;
    }

    let action = msg.action.clone()?;
    if action.starts_with("exec_") {
        return None;
    }
    let actor = msg.actor.as_ref();
    let attributes = actor.and_then(|a| a.attributes.as_ref());

    let attr = |key: &str| {
        attributes
            .and_then(|a| a.get(key))
            .cloned()
            .unwrap_or_default()
    };

    Some(ContainerEvent {
        ts: msg.time.unwrap_or(0),
        action,
        container_id: actor.and_then(|a| a.id.clone()).unwrap_or_default(),
        container_name: attr("name"),
        image: attr("image"),
        exit_code: attributes
            .and_then(|a| a.get("exitCode"))
            .and_then(|c| c.parse().ok()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::{EventActor, EventMessage, EventMessageTypeEnum};
    use std::collections::HashMap;

    fn container_event(action: &str, attrs: &[(&str, &str)]) -> EventMessage {
        EventMessage {
            typ: Some(EventMessageTypeEnum::CONTAINER),
            action: Some(action.to_string()),
            actor: Some(EventActor {
                id: Some("abc123".to_string()),
                attributes: Some(
                    attrs
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<HashMap<_, _>>(),
                ),
            }),
            time: Some(1_700_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn a_container_event_keeps_its_identity() {
        let ev = from_bollard(&container_event(
            "start",
            &[("name", "web-1"), ("image", "nginx:alpine")],
        ))
        .expect("container events are kept");

        assert_eq!(ev.action, "start");
        assert_eq!(ev.container_id, "abc123");
        assert_eq!(ev.container_name, "web-1");
        assert_eq!(ev.image, "nginx:alpine");
        assert_eq!(ev.ts, 1_700_000_000);
    }

    /// The exit code is the point of keeping `die` events: it separates a clean
    /// finish from a kill, which decides whether restarting is sensible.
    #[test]
    fn a_die_event_carries_its_exit_code() {
        let clean = from_bollard(&container_event("die", &[("exitCode", "0")])).expect("kept");
        assert_eq!(clean.exit_code, Some(0));

        let killed = from_bollard(&container_event("die", &[("exitCode", "137")])).expect("kept");
        assert_eq!(killed.exit_code, Some(137));
    }

    #[test]
    fn an_oom_kill_is_recognisable() {
        let oom = from_bollard(&container_event("oom", &[])).expect("kept");
        assert!(oom.is_oom());

        let ordinary = from_bollard(&container_event("die", &[("exitCode", "1")])).expect("kept");
        assert!(!ordinary.is_oom());
    }

    /// Healthcheck execs are container-type events and dominate the stream on a
    /// healthy machine — 18 of 33 in thirty seconds, measured. They are not
    /// lifecycle, and their `action` carries the full command line, which for an
    /// authenticating healthcheck means credentials in a `Serialize` struct.
    #[test]
    fn exec_events_are_dropped() {
        for action in [
            "exec_create: /bin/sh -c wget -qO- http://user:pw@127.0.0.1/health",
            "exec_start: /bin/sh -c true",
            "exec_die",
        ] {
            assert!(
                from_bollard(&container_event(action, &[])).is_none(),
                "exec event leaked: {action}"
            );
        }
    }

    /// Health transitions are lifecycle and must survive, despite sharing the
    /// `action: value` shape with the exec events above.
    #[test]
    fn health_status_changes_are_kept() {
        let ev = from_bollard(&container_event("health_status: unhealthy", &[]))
            .expect("health transitions are lifecycle");
        assert_eq!(ev.action, "health_status: unhealthy");
    }

    /// Image and network events ride the same stream and are not container
    /// lifecycle — a consumer counting restarts should never see them.
    #[test]
    fn non_container_events_are_dropped() {
        let image_pull = EventMessage {
            typ: Some(EventMessageTypeEnum::IMAGE),
            action: Some("pull".to_string()),
            ..Default::default()
        };
        assert!(from_bollard(&image_pull).is_none());
    }

    /// Docker omits fields depending on action and version; a missing name must
    /// not cost us the event.
    #[test]
    fn a_sparse_event_still_converts() {
        let bare = EventMessage {
            typ: Some(EventMessageTypeEnum::CONTAINER),
            action: Some("destroy".to_string()),
            ..Default::default()
        };
        let ev = from_bollard(&bare).expect("kept despite missing actor");
        assert_eq!(ev.action, "destroy");
        assert!(ev.container_id.is_empty());
        assert_eq!(ev.exit_code, None);
    }

    /// An event with no action is not usable, and inventing one would be worse
    /// than dropping it.
    #[test]
    fn an_actionless_event_is_dropped() {
        let no_action = EventMessage {
            typ: Some(EventMessageTypeEnum::CONTAINER),
            ..Default::default()
        };
        assert!(from_bollard(&no_action).is_none());
    }

    /// The property the whole module exists for: a burst arrives as a burst.
    /// The debounced path would deliver one refresh for all five.
    #[tokio::test]
    async fn a_rapid_burst_is_delivered_event_by_event() {
        let mut rx = subscribe();

        for i in 0..5 {
            publish(ContainerEvent {
                ts: 1_700_000_000 + i,
                action: "die".to_string(),
                container_id: "loop-1".to_string(),
                container_name: "crashy".to_string(),
                image: "busybox".to_string(),
                exit_code: Some(1),
            });
        }

        for i in 0..5 {
            let ev = rx.try_recv().expect("every event in the burst survives");
            assert_eq!(ev.ts, 1_700_000_000 + i);
        }
    }

    /// Publishing with nobody listening must not fail or block — the normal
    /// state before any consumer exists.
    #[test]
    fn publishing_without_subscribers_is_harmless() {
        publish(ContainerEvent {
            ts: 0,
            action: "start".to_string(),
            container_id: "nobody-listening".to_string(),
            container_name: String::new(),
            image: String::new(),
            exit_code: None,
        });
    }
}
