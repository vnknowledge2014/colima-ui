//! Deterministic compose fixers: pure text in, pure text out.
//!
//! Only three shapes are offered here, and the shortness of that list is the
//! result rather than a starting point. A fixability gate measured every
//! category against real `docker compose config` output and found that most
//! failures cannot be repaired without guessing at intent; these three can be
//! repaired from the error text alone, and all three survive their case probe
//! with comments intact.
//!
//! # Why text, not a parsed tree
//!
//! `serde_yml` loses every comment on round-trip. A user's compose file is
//! documentation as much as configuration, so a fix that silently strips their
//! notes is a bad trade for correcting one line. Editing the text directly
//! keeps the rest of the document byte-identical, which is also what makes the
//! resulting diff small enough to review.
//!
//! Each fixer returns the new file contents, a sentence explaining itself, and
//! the keys it means to remove. That last field feeds
//! [`super::compose_autofix::key_diff_check`]; a fixer that removes a key
//! without declaring it will have its own patch refused, which is the intent.

/// A candidate edit before it becomes a [`super::compose_autofix::Patch`].
pub struct Fix {
    pub new_content: String,
    pub explanation: String,
    pub confidence: f32,
    pub comments_preserved: bool,
    pub declared_removals: Vec<String>,
}

/// Replace tab indentation with spaces.
///
/// Tabs are never legal YAML indentation, so this needs no understanding of
/// the document at all — which is why it is the one `yaml_syntax` repair that
/// is safe to apply. Every other syntax error in the corpus required guessing
/// what the author meant.
pub fn fix_tabs(text: &str) -> Option<Fix> {
    if !text.contains('\t') {
        return None;
    }
    let mut changed = false;
    let fixed: Vec<String> = text
        .split('\n')
        .map(|line| {
            let tabs = line.chars().take_while(|c| *c == '\t').count();
            if tabs == 0 {
                return line.to_string();
            }
            changed = true;
            format!("{}{}", "  ".repeat(tabs), &line[tabs..])
        })
        .collect();

    if !changed {
        // Tabs exist, but inside values rather than as indentation. Rewriting
        // those would change data, not formatting.
        return None;
    }

    Some(Fix {
        new_content: fixed.join("\n"),
        explanation: "Replaced tab indentation with two spaces per level. YAML does not accept tabs for indentation.".to_string(),
        confidence: 0.95,
        comments_preserved: true,
        declared_removals: vec![],
    })
}

/// Declare top-level volumes and networks the file already refers to.
///
/// Nothing is invented: the names come out of Docker's own error message, and
/// an empty declaration is exactly what Compose documents for a default local
/// volume. Appended as text so the rest of the file, comments included, is
/// untouched.
pub fn fix_undefined_toplevel(text: &str, err: &str) -> Option<Fix> {
    let re = regex_lite::Regex::new(r#"(?i)undefined (volume|network)[:\s"]+([\w.-]+)"#).ok()?;

    let mut volumes: Vec<String> = vec![];
    let mut networks: Vec<String> = vec![];
    for caps in re.captures_iter(err) {
        let kind = caps.get(1)?.as_str().to_lowercase();
        let name = caps.get(2)?.as_str().to_string();
        let bucket = if kind == "volume" {
            &mut volumes
        } else {
            &mut networks
        };
        if !bucket.contains(&name) {
            bucket.push(name);
        }
    }
    if volumes.is_empty() && networks.is_empty() {
        return None;
    }

    let mut out = text.trim_end_matches('\n').to_string();
    let mut named: Vec<String> = vec![];
    for (section, names) in [("volumes", &volumes), ("networks", &networks)] {
        if names.is_empty() {
            continue;
        }
        // Appending a second `volumes:` mapping would make the document a
        // duplicate-key error instead of a fix. When the section already
        // exists, this is not a text append and is left to the user.
        if has_toplevel_section(text, section) {
            return None;
        }
        out.push_str(&format!("\n{}:\n", section));
        for name in names.iter() {
            out.push_str(&format!("  {}:\n", name));
            named.push(name.clone());
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }

    Some(Fix {
        new_content: out,
        explanation: format!(
            "Declared {} referenced by the file but never defined. An empty declaration creates a default local resource.",
            named.join(", ")
        ),
        confidence: 0.9,
        comments_preserved: true,
        declared_removals: vec![],
    })
}

/// True when `section` already exists as a top-level key.
fn has_toplevel_section(text: &str, section: &str) -> bool {
    text.lines().any(|line| {
        line.starts_with(section)
            && line[section.len()..].trim_start().starts_with(':')
    })
}

/// Wrap a scalar in a list where the schema demands an array.
///
/// The gate's verdict on `schema` errors was "coerce the type, never move the
/// key". Moving a key requires knowing where the author meant it to go; a
/// value that must be a list and is not is unambiguous.
pub fn fix_scalar_to_list(text: &str, err: &str) -> Option<Fix> {
    let key = regex_lite::Regex::new(r"(?i)(\w+) must be a (?:list|array)")
        .ok()?
        .captures(err)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .or_else(|| {
            regex_lite::Regex::new(r"(?i)services\.\w+\.(\w+) must be a")
                .ok()?
                .captures(err)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })?;

    let line_re =
        regex_lite::Regex::new(&format!(r"^(\s*){}:[ \t]+([^\n#]+?)[ \t]*$", regex_escape(&key)))
            .ok()?;

    let lines: Vec<&str> = text.split('\n').collect();
    let (idx, indent, value) = lines.iter().enumerate().find_map(|(i, line)| {
        let caps = line_re.captures(line)?;
        let indent = caps.get(1)?.as_str().to_string();
        let value = caps.get(2)?.as_str().trim().to_string();
        // Already structured, a block scalar, an anchor or an alias: all of
        // these mean the value is not a bare scalar and wrapping it would
        // corrupt it rather than coerce it.
        if value.starts_with(['[', '{', '|', '>', '&', '*']) {
            return None;
        }
        Some((i, indent, value))
    })?;

    // A value the author already quoted must not be quoted again: `"8080:80"`
    // wrapped a second time becomes the literal string `"8080:80"`, quotes and
    // all, which is a different value rather than the same one in a list.
    let quoted = if is_quoted(&value) {
        value.clone()
    } else {
        format!("\"{}\"", value)
    };

    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    out[idx] = format!("{}{}:\n{}  - {}", indent, key, indent, quoted);

    Some(Fix {
        new_content: out.join("\n"),
        explanation: format!(
            "`{}` must be a list. Wrapped the existing value in a single-item list; the value itself is unchanged.",
            key
        ),
        confidence: 0.85,
        comments_preserved: true,
        declared_removals: vec![],
    })
}

/// True when the value is already wrapped in a matching pair of quotes.
fn is_quoted(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
}

/// Escape the regex metacharacters that can appear in a compose key.
fn regex_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if "\\.+*?()|[]{}^$".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_become_spaces_and_comments_survive() {
        let src = "# keep me\nservices:\n\tweb:\n\t\timage: nginx\n";
        let fix = fix_tabs(src).expect("tab fix");
        assert!(fix.new_content.starts_with("# keep me\n"));
        assert!(!fix.new_content.contains('\t'));
        assert!(fix.new_content.contains("  web:"));
        assert!(fix.new_content.contains("    image: nginx"));
        assert!(fix.comments_preserved);
    }

    #[test]
    fn a_tab_inside_a_value_is_left_alone() {
        let src = "services:\n  web:\n    command: echo a\tb\n";
        assert!(fix_tabs(src).is_none());
    }

    #[test]
    fn undefined_volume_is_declared_from_the_error_text() {
        let src = "services:\n  db:\n    image: postgres\n    volumes:\n      - dbdata:/var/lib\n";
        let err = r#"service "db" refers to undefined volume dbdata: invalid compose project"#;
        let fix = fix_undefined_toplevel(src, err).expect("volume fix");
        assert!(fix.new_content.contains("volumes:\n  dbdata:\n"));
        // The service block is untouched.
        assert!(fix.new_content.contains("      - dbdata:/var/lib"));
    }

    #[test]
    fn an_existing_volumes_section_is_refused_rather_than_duplicated() {
        let src = "services:\n  db:\n    image: postgres\nvolumes:\n  other:\n";
        let err = r#"refers to undefined volume dbdata"#;
        assert!(fix_undefined_toplevel(src, err).is_none());
    }

    #[test]
    fn networks_and_volumes_are_declared_together() {
        let src = "services:\n  web:\n    image: nginx\n";
        let err = "refers to undefined volume data: and refers to undefined network backend:";
        let fix = fix_undefined_toplevel(src, err).expect("fix");
        assert!(fix.new_content.contains("volumes:\n  data:\n"));
        assert!(fix.new_content.contains("networks:\n  backend:\n"));
    }

    #[test]
    fn scalar_becomes_a_single_item_list() {
        let src = "services:\n  web:\n    image: nginx\n    ports: \"8080:80\"\n";
        let err = "services.web.ports must be a list";
        let fix = fix_scalar_to_list(src, err).expect("list fix");
        assert!(fix.new_content.contains("    ports:\n      - \"8080:80\""));
    }

    #[test]
    fn an_unquoted_scalar_gains_quotes() {
        let src = "services:\n  web:\n    ports: 8080:80\n";
        let fix = fix_scalar_to_list(src, "ports must be a list").expect("list fix");
        assert!(fix.new_content.contains("    ports:\n      - \"8080:80\""));
    }

    #[test]
    fn an_already_inline_list_is_not_rewrapped() {
        let src = "services:\n  web:\n    ports: [\"8080:80\"]\n";
        let err = "services.web.ports must be a list";
        assert!(fix_scalar_to_list(src, err).is_none());
    }

    #[test]
    fn an_anchor_is_not_treated_as_a_scalar() {
        let src = "services:\n  web:\n    ports: &ref\n";
        let err = "ports must be a list";
        assert!(fix_scalar_to_list(src, err).is_none());
    }
}
