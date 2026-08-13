//! The offline Help articles.
//!
//! These are the destination for the `doc_id` slugs that [`crate::error`] and
//! [`crate::commands::system_capabilities`] attach to failures: an error tells
//! the user *what* broke, an article tells them what to do about it.
//!
//! # Why the content is compiled in
//!
//! The markdown lives in `src-tauri/resources/kb/<locale>/<slug>.md` and is
//! pulled in with `include_str!`. Reading it from the resource directory at
//! runtime would work in the Tauri app, which has an `AppHandle` to resolve
//! resource paths — but the same code serves browser mode over HTTP, where
//! there is no handle and no guarantee the files shipped. Compiling them in
//! makes "works offline" a property of the binary rather than of the install.
//!
//! # Why they are then copied into SQLite
//!
//! Search. FTS5 does the ranking; a linear scan over `include_str!` constants
//! would not, and the article set is expected to grow.
//!
//! The seed is an upsert keyed on `(slug, locale)` and gated on
//! [`ARTICLE_VERSION`], so running it on every launch is a no-op until the
//! content actually changes.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::ColimaError;

/// Bump this when any article body changes. The seed only overwrites a stored
/// row whose version is lower, which is what lets a user's database survive an
/// app update without the content going stale.
const ARTICLE_VERSION: i64 = 2;

/// Locales with a full translation. A request for anything else falls back to
/// English rather than returning nothing.
pub const LOCALES: &[&str] = &["en", "vi", "ja", "zh"];

const FALLBACK_LOCALE: &str = "en";

/// `(slug, platform)`. Platform is advisory metadata for the UI — every article
/// is shown everywhere, but the badge tells a Linux user that a macOS-only
/// remedy will not apply to them.
const ARTICLE_META: &[(&str, &str)] = &[
    ("install-colima", "all"),
    ("start-colima", "all"),
    ("install-docker-cli", "all"),
    ("install-kubectl", "all"),
    ("install-trivy", "all"),
    ("common-errors", "all"),
    ("performance-tuning", "macos"),
    // Honeypot guides. Content only — there is no honeypot feature in the app,
    // and these deliberately stay articles rather than becoming one.
    ("honeypot-overview", "all"),
    ("honeypot-cowrie", "all"),
    ("honeypot-opencanary", "all"),
    ("honeypot-logs", "all"),
    // Falco install guide. Worth shipping on its own: the two traps it covers
    // (Homebrew installs a different `falco`, and Falco can run with zero
    // rules loaded) cost more time than the install itself.
];

/// `(slug, locale, body)` for every shipped article.
///
/// Spelled out rather than generated because `include_str!` needs a literal
/// path, and an explicit table is also the only thing that makes a missing
/// translation a compile error instead of a blank Help page.
const ARTICLE_BODIES: &[(&str, &str, &str)] = &[
    ("install-colima", "en", include_str!("../../resources/kb/en/install-colima.md")),
    ("install-colima", "vi", include_str!("../../resources/kb/vi/install-colima.md")),
    ("install-colima", "ja", include_str!("../../resources/kb/ja/install-colima.md")),
    ("install-colima", "zh", include_str!("../../resources/kb/zh/install-colima.md")),
    ("start-colima", "en", include_str!("../../resources/kb/en/start-colima.md")),
    ("start-colima", "vi", include_str!("../../resources/kb/vi/start-colima.md")),
    ("start-colima", "ja", include_str!("../../resources/kb/ja/start-colima.md")),
    ("start-colima", "zh", include_str!("../../resources/kb/zh/start-colima.md")),
    ("install-docker-cli", "en", include_str!("../../resources/kb/en/install-docker-cli.md")),
    ("install-docker-cli", "vi", include_str!("../../resources/kb/vi/install-docker-cli.md")),
    ("install-docker-cli", "ja", include_str!("../../resources/kb/ja/install-docker-cli.md")),
    ("install-docker-cli", "zh", include_str!("../../resources/kb/zh/install-docker-cli.md")),
    ("install-kubectl", "en", include_str!("../../resources/kb/en/install-kubectl.md")),
    ("install-kubectl", "vi", include_str!("../../resources/kb/vi/install-kubectl.md")),
    ("install-kubectl", "ja", include_str!("../../resources/kb/ja/install-kubectl.md")),
    ("install-kubectl", "zh", include_str!("../../resources/kb/zh/install-kubectl.md")),
    ("install-trivy", "en", include_str!("../../resources/kb/en/install-trivy.md")),
    ("install-trivy", "vi", include_str!("../../resources/kb/vi/install-trivy.md")),
    ("install-trivy", "ja", include_str!("../../resources/kb/ja/install-trivy.md")),
    ("install-trivy", "zh", include_str!("../../resources/kb/zh/install-trivy.md")),
    ("common-errors", "en", include_str!("../../resources/kb/en/common-errors.md")),
    ("common-errors", "vi", include_str!("../../resources/kb/vi/common-errors.md")),
    ("common-errors", "ja", include_str!("../../resources/kb/ja/common-errors.md")),
    ("common-errors", "zh", include_str!("../../resources/kb/zh/common-errors.md")),
    ("performance-tuning", "en", include_str!("../../resources/kb/en/performance-tuning.md")),
    ("performance-tuning", "vi", include_str!("../../resources/kb/vi/performance-tuning.md")),
    ("performance-tuning", "ja", include_str!("../../resources/kb/ja/performance-tuning.md")),
    ("performance-tuning", "zh", include_str!("../../resources/kb/zh/performance-tuning.md")),
    ("honeypot-overview", "en", include_str!("../../resources/kb/en/honeypot-overview.md")),
    ("honeypot-overview", "vi", include_str!("../../resources/kb/vi/honeypot-overview.md")),
    ("honeypot-overview", "ja", include_str!("../../resources/kb/ja/honeypot-overview.md")),
    ("honeypot-overview", "zh", include_str!("../../resources/kb/zh/honeypot-overview.md")),
    ("honeypot-cowrie", "en", include_str!("../../resources/kb/en/honeypot-cowrie.md")),
    ("honeypot-cowrie", "vi", include_str!("../../resources/kb/vi/honeypot-cowrie.md")),
    ("honeypot-cowrie", "ja", include_str!("../../resources/kb/ja/honeypot-cowrie.md")),
    ("honeypot-cowrie", "zh", include_str!("../../resources/kb/zh/honeypot-cowrie.md")),
    ("honeypot-opencanary", "en", include_str!("../../resources/kb/en/honeypot-opencanary.md")),
    ("honeypot-opencanary", "vi", include_str!("../../resources/kb/vi/honeypot-opencanary.md")),
    ("honeypot-opencanary", "ja", include_str!("../../resources/kb/ja/honeypot-opencanary.md")),
    ("honeypot-opencanary", "zh", include_str!("../../resources/kb/zh/honeypot-opencanary.md")),
    ("honeypot-logs", "en", include_str!("../../resources/kb/en/honeypot-logs.md")),
    ("honeypot-logs", "vi", include_str!("../../resources/kb/vi/honeypot-logs.md")),
    ("honeypot-logs", "ja", include_str!("../../resources/kb/ja/honeypot-logs.md")),
    ("honeypot-logs", "zh", include_str!("../../resources/kb/zh/honeypot-logs.md")),
];

#[derive(Debug, Clone, Serialize)]
pub struct Article {
    pub slug: String,
    pub locale: String,
    pub title: String,
    pub body: String,
    pub platform: String,
}

/// An article without its body — what the Help index and search results need.
#[derive(Debug, Clone, Serialize)]
pub struct ArticleSummary {
    pub slug: String,
    pub locale: String,
    pub title: String,
    pub platform: String,
    /// Search result context. Absent when listing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

/// The title is the document's H1 rather than a separate column, so a
/// translated article cannot end up with an untranslated title.
fn title_of(body: &str) -> &str {
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .unwrap_or("Untitled")
        .trim()
}

fn platform_of(slug: &str) -> &'static str {
    ARTICLE_META
        .iter()
        .find(|(s, _)| *s == slug)
        .map_or("all", |(_, p)| *p)
}

/// Normalize whatever the frontend sent into a locale we actually have.
fn resolve_locale(requested: &str) -> &str {
    // Accept "vi-VN" and "zh_CN" as well as bare tags.
    let base = requested
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    LOCALES
        .iter()
        .find(|l| **l == base)
        .copied()
        .unwrap_or(FALLBACK_LOCALE)
}

/// Insert or refresh every shipped article. Idempotent: running it repeatedly
/// leaves the table byte-identical once the stored version matches.
pub fn seed(conn: &Connection) {
    for (slug, locale, body) in ARTICLE_BODIES {
        let result = conn.execute(
            "INSERT INTO articles (slug, locale, title, body, platform, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(slug, locale) DO UPDATE SET
                 title = excluded.title,
                 body = excluded.body,
                 platform = excluded.platform,
                 version = excluded.version,
                 updated_at = datetime('now')
             WHERE excluded.version > articles.version",
            params![
                slug,
                locale,
                title_of(body),
                body,
                platform_of(slug),
                ARTICLE_VERSION
            ],
        );
        if let Err(e) = result {
            // A failed article seed must not stop the app from starting; the
            // Help page degrades, nothing else does.
            eprintln!("[kb_articles] failed to seed {}/{}: {}", slug, locale, e);
        }
    }
}

fn db() -> &'static std::sync::Mutex<Connection> {
    super::knowledge_bank::get_db()
}

fn db_err(e: rusqlite::Error) -> ColimaError {
    ColimaError::internal(format!("Knowledge base query failed: {}", e))
}

/// Every article for a locale, ordered as the Help sidebar shows them.
#[tauri::command]
pub async fn kb_list_articles(locale: String) -> Result<Vec<ArticleSummary>, ColimaError> {
    let locale = resolve_locale(&locale).to_string();
    let conn = db().lock().map_err(|_| ColimaError::internal("KB lock poisoned"))?;

    let mut stmt = conn
        .prepare("SELECT slug, locale, title, platform FROM articles WHERE locale = ?1")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![locale], |r| {
            Ok(ArticleSummary {
                slug: r.get(0)?,
                locale: r.get(1)?,
                title: r.get(2)?,
                platform: r.get(3)?,
                excerpt: None,
            })
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;

    // Order in Rust by the shipped table so the sidebar reads install → start →
    // troubleshoot, which SQL cannot express without a sort column.
    let mut rows = rows;
    rows.sort_by_key(|a| {
        ARTICLE_META
            .iter()
            .position(|(s, _)| *s == a.slug)
            .unwrap_or(usize::MAX)
    });
    Ok(rows)
}

/// One article, falling back to English when the requested locale has no
/// translation of it.
#[tauri::command]
pub async fn kb_get_article(slug: String, locale: String) -> Result<Article, ColimaError> {
    let requested = resolve_locale(&locale).to_string();
    let conn = db().lock().map_err(|_| ColimaError::internal("KB lock poisoned"))?;

    let fetch = |locale: &str| -> Result<Option<Article>, ColimaError> {
        conn.query_row(
            "SELECT slug, locale, title, body, platform FROM articles WHERE slug = ?1 AND locale = ?2",
            params![slug, locale],
            |r| {
                Ok(Article {
                    slug: r.get(0)?,
                    locale: r.get(1)?,
                    title: r.get(2)?,
                    body: r.get(3)?,
                    platform: r.get(4)?,
                })
            },
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(db_err(other)),
        })
    };

    if let Some(article) = fetch(&requested)? {
        return Ok(article);
    }
    if requested != FALLBACK_LOCALE {
        if let Some(article) = fetch(FALLBACK_LOCALE)? {
            return Ok(article);
        }
    }
    Err(ColimaError::not_found(format!(
        "No Help article named {:?}",
        slug
    )))
}

/// Full-text search within one locale.
#[tauri::command]
pub async fn kb_search_articles(
    query: String,
    locale: String,
) -> Result<Vec<ArticleSummary>, ColimaError> {
    let locale = resolve_locale(&locale).to_string();
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // FTS5 treats punctuation as query syntax, so a user typing `x509:` or an
    // unbalanced quote would otherwise get a parse error instead of results.
    // Quoting each token makes the whole input a literal phrase search.
    let fts_query = trimmed
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");

    let conn = db().lock().map_err(|_| ColimaError::internal("KB lock poisoned"))?;
    let mut stmt = conn
        .prepare(
            "SELECT a.slug, a.locale, a.title, a.platform,
                    snippet(articles_fts, 1, '', '', '…', 24)
             FROM articles_fts f
             JOIN articles a ON a.id = f.rowid
             WHERE articles_fts MATCH ?1 AND a.locale = ?2
             ORDER BY rank
             LIMIT 20",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map(params![fts_query, locale], |r| {
            Ok(ArticleSummary {
                slug: r.get(0)?,
                locale: r.get(1)?,
                title: r.get(2)?,
                platform: r.get(3)?,
                excerpt: r.get::<_, Option<String>>(4)?,
            })
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE articles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slug TEXT NOT NULL,
                locale TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                platform TEXT NOT NULL DEFAULT 'all',
                version INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(slug, locale)
            );
            CREATE VIRTUAL TABLE articles_fts USING fts5(
                title, body, content='articles', content_rowid='id'
            );
            CREATE TRIGGER articles_ai_fts AFTER INSERT ON articles BEGIN
                INSERT INTO articles_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
            END;
            CREATE TRIGGER articles_ad_fts AFTER DELETE ON articles BEGIN
                INSERT INTO articles_fts(articles_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
            END;
            CREATE TRIGGER articles_au_fts AFTER UPDATE ON articles BEGIN
                INSERT INTO articles_fts(articles_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
                INSERT INTO articles_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
            END;",
        )
        .expect("create schema");
        conn
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM articles", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn seeding_three_times_does_not_duplicate() {
        let conn = memory_db();
        seed(&conn);
        let after_first = count(&conn);
        seed(&conn);
        seed(&conn);
        assert_eq!(count(&conn), after_first);
        assert_eq!(after_first as usize, ARTICLE_BODIES.len());
    }

    #[test]
    fn reseeding_does_not_rewrite_rows_at_the_same_version() {
        let conn = memory_db();
        seed(&conn);
        conn.execute("UPDATE articles SET body = 'edited' WHERE slug = 'common-errors' AND locale = 'en'", [])
            .unwrap();
        seed(&conn);
        let body: String = conn
            .query_row(
                "SELECT body FROM articles WHERE slug = 'common-errors' AND locale = 'en'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            body, "edited",
            "seed must not overwrite a row already at ARTICLE_VERSION"
        );
    }

    #[test]
    fn every_slug_has_every_locale() {
        for (slug, _) in ARTICLE_META {
            for locale in LOCALES {
                assert!(
                    ARTICLE_BODIES
                        .iter()
                        .any(|(s, l, _)| s == slug && l == locale),
                    "missing translation {}/{}",
                    slug,
                    locale
                );
            }
        }
        assert_eq!(ARTICLE_BODIES.len(), ARTICLE_META.len() * LOCALES.len());
    }

    /// The whole point of the slugs: an error that names a `doc_id` has to open
    /// something. These are the slugs `error.rs` and `system_capabilities.rs`
    /// emit today.
    #[test]
    fn every_referenced_doc_id_has_an_article() {
        for slug in [
            "start-colima",
            "install-colima",
            "install-docker-cli",
            "install-kubectl",
            "install-trivy",
            "common-errors",
        ] {
            assert!(
                ARTICLE_META.iter().any(|(s, _)| *s == slug),
                "doc_id {:?} has no article",
                slug
            );
        }
    }

    #[test]
    fn titles_come_from_the_h1() {
        assert_eq!(title_of("# Hello\n\nbody\n"), "Hello");
        assert_eq!(title_of("no heading"), "Untitled");
        for (slug, locale, body) in ARTICLE_BODIES {
            assert_ne!(
                title_of(body),
                "Untitled",
                "{}/{} has no H1",
                slug,
                locale
            );
        }
    }

    #[test]
    fn locale_tags_are_normalized_with_english_fallback() {
        assert_eq!(resolve_locale("vi"), "vi");
        assert_eq!(resolve_locale("zh-CN"), "zh");
        assert_eq!(resolve_locale("ja_JP"), "ja");
        assert_eq!(resolve_locale("de"), "en");
        assert_eq!(resolve_locale(""), "en");
    }

    #[test]
    fn fts_finds_seeded_content() {
        let conn = memory_db();
        seed(&conn);
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM articles_fts f JOIN articles a ON a.id = f.rowid
                 WHERE articles_fts MATCH '\"virtiofs\"' AND a.locale = 'en'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(hits > 0, "virtiofs should be findable in the English set");
    }
}
