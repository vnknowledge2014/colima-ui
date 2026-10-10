//! Alternative base images, as a static catalog.
//!
//! The question a low score provokes is "then what should I use instead?", and
//! this file is the answer — a table shipped with the app, not a service.
//!
//! ## Why a downloaded table rather than a query
//!
//! Asking a server "how does image X score?" would send the user's list of
//! images to us. That list says what they run, for whom, and sometimes what they
//! are building before it is public. It is also a bill that grows with usage.
//! A table the app already has answers the same question and sends nothing.
//!
//! ## Why the wording is careful
//!
//! Each entry says what a swap *changes*, not what the reader should do. Whether
//! Alpine is right for a service depends on whether its dependencies build
//! against musl — which this file cannot know, and pretending otherwise would
//! make the advice worse than none.
//!
//! ## Signed updates are not here yet
//!
//! Phase 3 of the plan pairs this with a signed update channel
//! (`catalog-vN.json` + `.sig`, verified before parsing). That needs a private
//! key and a publishing process, which is an open question for the owner — see
//! the plan's Unresolved #1. Until it is answered, the embedded catalog is the
//! only one, which is also the fallback the update path would need anyway.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

const CATALOG_JSON: &str = include_str!("../../resources/catalog/v1.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alternative {
    /// May contain a `{version}` placeholder, filled in from the image the user
    /// actually has — suggesting `python:3.12-slim` to someone on 3.11 would be
    /// two changes disguised as one.
    pub image: String,
    /// What changes if you swap. Not an instruction.
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub base_family: String,
    pub alternatives: Vec<Alternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub catalog_version: String,
    /// When this table was last revised. Shown in the UI: advice nobody has
    /// looked at for a year should say so rather than look current.
    pub updated_at: String,
    pub entries: HashMap<String, CatalogEntry>,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    serde_json::from_str(CATALOG_JSON).expect("the embedded catalog must parse")
});

pub fn catalog() -> &'static Catalog {
    &CATALOG
}

/// The catalog key for an image reference.
///
/// `docker.io/library/python:3.12-slim` and `python:3.12` are the same entry;
/// a private image nobody publishes has none.
fn repository_key(image_ref: &str) -> String {
    let without_digest = image_ref.split('@').next().unwrap_or(image_ref);
    let last = without_digest.rsplit('/').next().unwrap_or(without_digest);
    let repo = last.split(':').next().unwrap_or(last);
    repo.to_ascii_lowercase()
}

/// The version part of a tag, when there is one.
///
/// `3.12-slim` → `3.12`, `20-alpine` → `20`, `latest` → nothing. Used to keep a
/// suggestion on the version the user is already running.
fn version_from_tag(image_ref: &str) -> Option<String> {
    let without_digest = image_ref.split('@').next().unwrap_or(image_ref);
    let last = without_digest.rsplit('/').next().unwrap_or(without_digest);
    let (_, tag) = last.split_once(':')?;
    let head = tag.split('-').next()?;
    let looks_like_version = !head.is_empty()
        && head.chars().next().is_some_and(|c| c.is_ascii_digit())
        && head.chars().all(|c| c.is_ascii_digit() || c == '.');
    looks_like_version.then(|| head.to_string())
}

/// The tag part of a reference, or `latest` when it is implied.
fn tag_of(image_ref: &str) -> String {
    let without_digest = image_ref.split('@').next().unwrap_or(image_ref);
    let last = without_digest.rsplit('/').next().unwrap_or(without_digest);
    last.split_once(':')
        .map_or_else(|| "latest".to_string(), |(_, tag)| tag.to_ascii_lowercase())
}

/// Whether two references name the same image, ignoring how they are spelled.
fn same_image(a: &str, b: &str) -> bool {
    repository_key(a) == repository_key(b) && tag_of(a) == tag_of(b)
}

/// Alternatives for one image, with `{version}` resolved where possible.
///
/// An entry whose suggestion still contains `{version}` after substitution is
/// dropped: a literal `python:{version}-slim` in the UI is worse than showing
/// one fewer option.
pub fn alternatives_for(image_ref: &str) -> Vec<Alternative> {
    let Some(entry) = catalog().entries.get(&repository_key(image_ref)) else {
        return Vec::new();
    };
    let version = version_from_tag(image_ref);
    entry
        .alternatives
        .iter()
        .filter_map(|alt| {
            let image = match &version {
                Some(v) => alt.image.replace("{version}", v),
                None if alt.image.contains("{version}") => return None,
                None => alt.image.clone(),
            };
            // Suggesting exactly what the user already has is noise — compared
            // after normalisation, because `docker.io/library/debian:stable-slim`
            // and `debian:stable-slim` are the same image with different
            // spellings, and only one of them would be caught by `==`.
            if same_image(&image, image_ref) {
                return None;
            }
            Some(Alternative {
                image,
                why: alt.why.clone(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSuggestions {
    pub catalog_version: String,
    pub updated_at: String,
    pub alternatives: Vec<Alternative>,
}

#[tauri::command]
pub async fn security_alternatives(
    image_ref: String,
) -> Result<CatalogSuggestions, crate::error::ColimaError> {
    Ok(suggestions_for(&image_ref))
}

pub fn suggestions_for(image_ref: &str) -> CatalogSuggestions {
    CatalogSuggestions {
        catalog_version: catalog().catalog_version.clone(),
        updated_at: catalog().updated_at.clone(),
        alternatives: alternatives_for(image_ref),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_catalog_parses_and_says_something_useful() {
        let c = catalog();
        assert!(c.entries.len() >= 20, "a catalog this thin is not worth shipping");
        for (name, entry) in &c.entries {
            assert!(!entry.alternatives.is_empty(), "{} has no alternatives", name);
            for alt in &entry.alternatives {
                assert!(!alt.why.trim().is_empty(), "{} has a suggestion with no reason", name);
                // The wording rule from the plan: say what changes, not what to
                // do. Imperatives creep in one entry at a time.
                let why = alt.why.to_lowercase();
                assert!(
                    !why.contains("you should"),
                    "{} tells the reader what to do: {}",
                    name,
                    alt.why
                );
            }
        }
    }

    #[test]
    fn a_registry_prefix_and_a_digest_do_not_hide_the_entry() {
        assert_eq!(repository_key("python:3.12"), "python");
        assert_eq!(repository_key("docker.io/library/python:3.12-slim"), "python");
        assert_eq!(repository_key("PYTHON@sha256:abc"), "python");
        assert_eq!(repository_key("registry.test:5000/team/app:1"), "app");
    }

    #[test]
    fn suggestions_keep_the_version_the_user_is_already_on() {
        // Suggesting 3.12-slim to somebody running 3.11 is two changes wearing
        // the disguise of one, and the second is the one that breaks the build.
        let alts = alternatives_for("python:3.11");
        assert!(alts.iter().any(|a| a.image == "python:3.11-slim"), "{:?}", alts);
        assert!(alts.iter().all(|a| !a.image.contains("{version}")));

        let alts = alternatives_for("node:22-alpine");
        assert!(alts.iter().any(|a| a.image == "node:22-slim"));
        // The image the user already has is not a suggestion.
        assert!(alts.iter().all(|a| a.image != "node:22-alpine"));
    }

    #[test]
    fn the_image_the_user_runs_is_not_suggested_however_it_is_spelled() {
        // Same image, three spellings. A verbatim comparison catches one.
        for spelling in [
            "debian:stable-slim",
            "docker.io/library/debian:stable-slim",
            "DEBIAN:STABLE-SLIM",
        ] {
            let alts = alternatives_for(spelling);
            assert!(
                alts.iter().all(|a| !a.image.eq_ignore_ascii_case("debian:stable-slim")),
                "{} was offered itself: {:?}",
                spelling,
                alts
            );
        }
    }

    #[test]
    fn an_unversioned_tag_drops_the_suggestions_it_cannot_fill_in() {
        let alts = alternatives_for("python:latest");
        assert!(alts.iter().all(|a| !a.image.contains("{version}")));
        // The distroless entry carries no placeholder, so it survives.
        assert!(alts.iter().any(|a| a.image.contains("distroless")));
    }

    #[test]
    fn an_image_nobody_publishes_gets_no_advice_rather_than_wrong_advice() {
        let s = suggestions_for("my-company/internal-api:2.3.1");
        assert!(s.alternatives.is_empty());
        // The catalog still identifies itself, so the UI can say "nothing known
        // about this image" rather than "no catalog".
        assert!(!s.catalog_version.is_empty());
        assert!(!s.updated_at.is_empty());
    }
}
