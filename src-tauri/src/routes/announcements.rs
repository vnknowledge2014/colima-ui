//! Browser-mode entry point for the announcement feed.
//!
//! Same fetch as the Tauri command; the frontend calls whichever transport it has.
//! Reachable with a valid token like every other route — not because the feed is
//! secret (it is a public file) but because the local API is not a public proxy for
//! arbitrary hosts, and this keeps that true by construction.

use axum::{http::StatusCode, response::Json};

use crate::api_server::*;
use crate::commands::announcements::{self, AnnouncementFeed};

pub async fn api_announcements(
) -> (StatusCode, Json<ApiResponse<AnnouncementFeed>>) {
    match announcements::fetch_feed().await {
        Ok(feed) => ok(feed),
        // Reported as an error, never as an empty feed: the client keeps what it
        // already showed rather than clearing the list on a flaky network.
        Err(e) => err(e),
    }
}
