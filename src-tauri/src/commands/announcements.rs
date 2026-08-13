//! Announcements from the vendor: releases, advisories, maintenance notices.
//!
//! One static JSON file, fetched here rather than in the webview.
//!
//! ## Why the backend fetches it
//!
//! `tauri.conf.json` keeps `connect-src` tight and does not list the feed's host.
//! The alternatives were to widen the CSP for a P3 feature, or to let the webview
//! use the HTTP plugin — whose capability currently allows `https://*`, i.e. every
//! host on the internet. Fetching here costs neither: the CSP is untouched and
//! exactly one hard-coded URL is reachable.
//!
//! It also matches how the rest of the app is built. The frontend talks to our API;
//! our API talks to the world.
//!
//! ## What deliberately does not happen
//!
//! No user id, no machine id, no cookies, no custom headers — a bare GET. The
//! request still reveals an IP, a timestamp and the app version to the host, which
//! is why this is documented in `docs/telemetry.md` and can be switched off. What
//! it must never do is carry anything *about the user*.
//!
//! Nothing is cached to disk. The feed is small and polled every few hours; a cache
//! would be state that can go stale for no benefit.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::ColimaError;

/// Where the feed lives.
///
/// A file in the repository, so publishing is a commit and reviewing content is
/// code review — no extra infrastructure and no second approval process.
/// Hard-coded and parameterless: there is no way for a caller to turn this command
/// into a request for somewhere else.
const FEED_URL: &str =
    "https://raw.githubusercontent.com/vnknowledge2014/colima-ui/main/announcements.json";

/// The newest feed shape this build understands.
const SUPPORTED_VERSION: u32 = 1;

/// Give up rather than hold a request open. Nobody is waiting on this.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Refuse a body that is not plausibly this feed.
///
/// The feed is a few kilobytes. This is not a security boundary — a hostile host
/// controls the content either way — it stops a wrong URL or a captive portal from
/// being read into memory in full.
const MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    /// Stable, human-authored. The client remembers which ids it has shown.
    pub id: String,
    pub published_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    /// `info` | `warning` | `critical`. Not an enum: an unknown severity from a
    /// newer feed must not fail the whole parse, and the client treats anything it
    /// does not recognise as `info`.
    pub severity: String,
    /// `null` = everyone. Filtered on the client, which is the only side that knows
    /// whether this install is paid.
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub max_version: Option<String>,
    /// Locale code → text. The app ships four languages; a single string would
    /// force English on all of them.
    pub title: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub link_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementFeed {
    pub version: u32,
    pub announcements: Vec<Announcement>,
}

/// Fetch the feed.
///
/// Returns `Err` when the feed could not be read — deliberately not an empty feed.
/// "There is nothing to announce" and "I could not find out" look identical to a
/// caller that cannot tell them apart, and the client needs the difference: on
/// failure it keeps showing what it already had rather than clearing the list.
#[tauri::command]
pub async fn announcements_fetch() -> Result<AnnouncementFeed, ColimaError> {
    Ok(fetch_feed().await?)
}

pub async fn fetch_feed() -> Result<AnnouncementFeed, String> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        // No cookie store: this request carries nothing that identifies anyone, and
        // a jar is the easiest way for that to stop being true.
        .build()
        .map_err(|e| format!("Cannot build HTTP client: {}", e))?;

    let response = client
        .get(FEED_URL)
        .send()
        .await
        .map_err(|e| crate::redact::redact_err("Announcements unavailable", e))?;

    if !response.status().is_success() {
        return Err(format!("Announcements unavailable: HTTP {}", response.status()));
    }

    // Checked before reading where the server declares it, so an oversized body is
    // usually refused without transferring it at all.
    if let Some(len) = response.content_length() {
        if len as usize > MAX_BYTES {
            return Err(format!("Announcement feed is too large: {} bytes", len));
        }
    }

    let body = response
        .bytes()
        .await
        .map_err(|e| crate::redact::redact_err("Announcements unavailable", e))?;
    // Checked again: `content-length` is a claim, not a guarantee.
    if body.len() > MAX_BYTES {
        return Err(format!("Announcement feed is too large: {} bytes", body.len()));
    }

    parse_feed(&body)
}

/// Parse and version-check.
///
/// Unknown fields are ignored rather than rejected: a feed written for a later
/// build must not break this one. A newer *version* is refused outright, because
/// that is the signal that the shape itself changed and guessing would be worse
/// than showing nothing.
pub fn parse_feed(body: &[u8]) -> Result<AnnouncementFeed, String> {
    let feed: AnnouncementFeed = serde_json::from_slice(body)
        .map_err(|e| format!("Announcement feed is not readable: {}", e))?;

    if feed.version > SUPPORTED_VERSION {
        return Err(format!(
            "Announcement feed is version {}, this build understands {}",
            feed.version, SUPPORTED_VERSION
        ));
    }
    Ok(feed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_feed() {
        let json = br#"{
            "version": 1,
            "announcements": [{
                "id": "2026-08-release",
                "publishedAt": "2026-08-12T00:00:00Z",
                "severity": "info",
                "title": { "en": "ColimaUI 0.2.0", "vi": "ColimaUI 0.2.0" },
                "linkUrl": "https://github.com/vnknowledge2014/colima-ui/releases"
            }]
        }"#;

        let feed = parse_feed(json).expect("a well-formed feed must parse");
        assert_eq!(feed.announcements.len(), 1);
        assert_eq!(feed.announcements[0].title["en"], "ColimaUI 0.2.0");
        // Optional fields absent from the document, not absent from the type.
        assert!(feed.announcements[0].audience.is_none());
        assert!(feed.announcements[0].body.is_none());
    }

    #[test]
    fn a_newer_feed_does_not_break_an_older_build() {
        // The publisher adds a field this build has never heard of. Refusing here
        // would mean one feed edit could silence announcements for everyone who has
        // not updated — including the advisory telling them to update.
        let json = br#"{
            "version": 1,
            "announcements": [{
                "id": "x",
                "publishedAt": "2026-08-12T00:00:00Z",
                "severity": "warning",
                "title": { "en": "Hello" },
                "dismissible": true,
                "icon": "sparkles"
            }]
        }"#;

        assert!(parse_feed(json).is_ok());
    }

    #[test]
    fn a_newer_shape_is_refused_rather_than_guessed() {
        let json = br#"{ "version": 99, "announcements": [] }"#;
        let err = parse_feed(json).expect_err("an unknown shape must not be guessed at");
        assert!(err.contains("version 99"), "unexpected: {}", err);
    }

    #[test]
    fn an_empty_feed_is_valid() {
        // The default state of the file in the repository. It has to parse, or the
        // feature is broken until the first announcement is written.
        let feed = parse_feed(br#"{ "version": 1, "announcements": [] }"#).unwrap();
        assert!(feed.announcements.is_empty());
    }

    #[test]
    fn rubbish_is_reported_as_unreadable() {
        assert!(parse_feed(b"<!DOCTYPE html><html>404</html>").is_err());
    }

    #[test]
    fn an_unknown_severity_survives_parsing() {
        // Severity is a string, not an enum, so a newer vocabulary reaches the
        // client — which downgrades what it does not recognise — instead of
        // failing the whole document here.
        let json = br#"{
            "version": 1,
            "announcements": [{
                "id": "x",
                "publishedAt": "2026-08-12T00:00:00Z",
                "severity": "catastrophic",
                "title": { "en": "Hello" }
            }]
        }"#;
        let feed = parse_feed(json).unwrap();
        assert_eq!(feed.announcements[0].severity, "catastrophic");
    }
}
