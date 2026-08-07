use serde::{Deserialize, Serialize};

// Shared HTTP client — equivalent to Python's requests.Session()
// Maintains cookies, connection pooling, and default headers across all requests.
// Reference: kuosuko/ollama-web-search WebSearchAssistant.__init__()
fn http_client() -> reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .cookie_store(true) // ← like requests.Session() — persist cookies across calls
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .build()
                .unwrap_or_default()
        })
        .clone()
}

// ===== Data Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
    pub engine: String,
}

/// Default SearXNG instances — tried in order, first success wins.
/// Local instance is fastest and most reliable (no rate limits).
/// Public instances from kuosuko/ollama-web-search as fallback.
/// DuckDuckGo HTML scraping is the final fallback if all SearXNG fail.
const DEFAULT_SEARXNG_INSTANCES: &[&str] = &[
    "http://localhost:8888/search",          // Local SearXNG container (most reliable)
    "https://search.inetol.net/search",      // Public fallback
    "https://searx.be/search",
    "https://search.brave4u.com/search",
    "https://priv.au/search",
];

#[tauri::command]
pub async fn searxng_search(     query: String,     instances: Option<Vec<String>>,     max_results: Option<usize>,     timeout_secs: Option<u64>, ) -> Result<Vec<SearchResult>, crate::error::ColimaError> {
    async move {
    let instances: Vec<String> = instances
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_SEARXNG_INSTANCES.iter().map(|s| s.to_string()).collect()
        });
    let max_results = max_results.unwrap_or(8);

    // Use shared client (session) — same as Python's self.session.get()
    // The session maintains cookies across calls, which is key for SearXNG
    let client = if let Some(secs) = timeout_secs {
        // If custom timeout, create a one-off client (still with cookies)
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(secs))
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .build()
            .unwrap_or_default()
    } else {
        http_client() // shared session with cookies
    };

    let mut last_error = String::from("No SearXNG instances configured");

    // Try each instance with fallback — 1:1 from browse_web() in main.py
    for instance in &instances {
        let base = instance.trim_end_matches('/');
        // Match Python: f"{instance}?q={query}&format=json&categories=general"
        let url = if base.contains('?') {
            format!("{}&q={}&format=json", base, urlencoding(&query))
        } else {
            format!("{}?q={}&format=json&categories=general", base, urlencoding(&query))
        };

        // Python: response = self.session.get(search_url, timeout=CONFIG['timeout'])
        // Note: Python does NOT set Accept header — we match that behavior
        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_error = format!("SearXNG {} returned status {}", base, resp.status());
                    continue;
                }
                // Python: data = response.json(); results = data.get('results', [])
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let results: Vec<SearchResult> = json["results"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .take(max_results)
                                    .filter_map(|r| {
                                        Some(SearchResult {
                                            title: r["title"].as_str()?.to_string(),
                                            url: r["url"].as_str()?.to_string(),
                                            content: r["content"].as_str().unwrap_or("").to_string(),
                                            engine: r["engine"].as_str().unwrap_or("unknown").to_string(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        // Python: if results: return results[:CONFIG['max_results']]
                        if !results.is_empty() {
                            return Ok(results);
                        }
                        last_error = format!("SearXNG {} returned 0 results", base);
                    }
                    Err(e) => {
                        last_error = format!("SearXNG {} JSON parse error: {}", base, e);
                    }
                }
            }
            // Python: except Exception as e: continue
            Err(e) => {
                last_error = format!("SearXNG {} connection failed: {}", base, e);
            }
        }
    }

    // ===== Fallback: DuckDuckGo HTML Lite scraping =====
    // When all SearXNG instances fail (429/403), scrape DuckDuckGo Lite
    // as a zero-config fallback. No API key needed.
    match duckduckgo_fallback(&query, max_results).await {
        Ok(results) if !results.is_empty() => return Ok(results),
        Ok(_) => { /* empty results, fall through */ }
        Err(e) => {
            last_error = format!("All SearXNG instances failed. DuckDuckGo fallback also failed: {}", e);
        }
    }

    Err(last_error)
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

/// DuckDuckGo Lite HTML scraping fallback
/// Uses POST to lite.duckduckgo.com (same as the HTML form submission)
async fn duckduckgo_fallback(query: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default();

    // DDG Lite requires POST with form-encoded body (just like their HTML form)
    let resp = client
        .post("https://lite.duckduckgo.com/lite/")
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Referer", "https://lite.duckduckgo.com/")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "text/html")
        .body(format!("q={}&df=&kl=&kp=", urlencoding(query)))
        .send()
        .await
        .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("DuckDuckGo returned status {}", resp.status()));
    }

    let html = resp.text().await.map_err(|e| format!("DuckDuckGo read error: {}", e))?;
    let doc = scraper::Html::parse_document(&html);
    let mut results: Vec<SearchResult> = Vec::new();

    // Parse a.result-link elements (DDG Lite format: title + href)
    if let Ok(link_sel) = scraper::Selector::parse("a.result-link") {
        for el in doc.select(&link_sel).take(max_results + 5) {
            let title = el.text().collect::<String>().trim().to_string();
            let href = el.value().attr("href").unwrap_or("").to_string();
            if !title.is_empty() && !href.is_empty() && href.starts_with("http") {
                results.push(SearchResult {
                    title,
                    url: href,
                    content: String::new(),
                    engine: "duckduckgo".to_string(),
                });
            }
        }
    }

    // Parse td.result-snippet elements (snippets paired with links)
    if let Ok(snippet_sel) = scraper::Selector::parse("td.result-snippet") {
        for (i, el) in doc.select(&snippet_sel).enumerate() {
            if i < results.len() {
                results[i].content = el.text().collect::<String>().trim().to_string();
            }
        }
    }

    // Deduplicate by URL
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.url.clone()));
    results.truncate(max_results);

    Ok(results)
}

/// Simple URL encoding for query strings
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

// ===== Jina Reader API (from kuosuko/ollama-web-search) =====
// Free API that returns clean markdown/text from any URL.
// Reference: retrieve_page_information() in main.py

async fn jina_reader_fetch(url: &str, max_length: usize) -> Result<String, String> {
    let jina_url = format!("https://r.jina.ai/{}", url);

    let resp = http_client()
        .get(&jina_url)
        .header("Accept", "text/plain")
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("Jina Reader failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Jina Reader returned status {}", resp.status()));
    }

    let content = resp.text().await.map_err(|e| format!("Jina read error: {}", e))?;
    let content = content.trim().to_string();

    // Truncate if needed (same as repo: 10000 char limit)
    if content.len() > max_length {
        let truncated = &content[..max_length];
        let cut = truncated.rfind('\n').unwrap_or(max_length);
        Ok(format!("{}\n\n[...truncated at {} chars]", &content[..cut], cut))
    } else {
        Ok(content)
    }
}

// ===== Command 2: Fetch Page as Clean Markdown =====
// Strategy: Try Jina Reader API first (free, clean output — from kuosuko/ollama-web-search),
// then fall back to direct HTML scraping + html2md pipeline.

#[tauri::command]
pub async fn fetch_page_as_markdown(     url: String,     max_length: Option<usize>,     mode: Option<String>, ) -> Result<String, crate::error::ColimaError> {
    async move {
    let max_length = max_length.unwrap_or(8000);
    let mode = mode.unwrap_or_else(|| "full".to_string());

    // Strategy 1: Jina Reader API (https://r.jina.ai/) — free, returns clean markdown
    // Reference: kuosuko/ollama-web-search retrieve_page_information()
    if let Ok(content) = jina_reader_fetch(&url, max_length).await {
        if content.len() > 100 {
            return Ok(content);
        }
    }

    // Strategy 2: Direct HTML scraping + html2md (fallback)
    let resp = http_client()
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {}", resp.status(), url));
    }

    let html = resp.text().await.map_err(|e| format!("Read error: {}", e))?;

    // Extract metadata (title)
    let title = extract_title(&html);

    // Extract main content (scraper — Readability equivalent)
    let clean_html = extract_main_content(&html);

    // Convert to markdown (html2md — Turndown equivalent)
    let markdown = html2md::parse_html(&clean_html);

    // 5. Post-process (1:1 from MD-This-Page markdown.tsx)
    let result = post_process_markdown(&markdown, &title, &url, &mode);

    // 6. Truncate if needed
    if result.len() > max_length {
        let truncated = &result[..max_length];
        // Find last newline to avoid cutting mid-line
        let cut = truncated.rfind('\n').unwrap_or(max_length);
        Ok(format!("{}\n\n[...truncated at {} chars]", &result[..cut], cut))
    } else {
        Ok(result)
    }
    }
    .await.map_err(|e: String| crate::error::ColimaError::from(e))
}

// ===== Content Extraction (Readability equivalent) =====
// Reference: Defuddle.parse() from MD-This-Page content.ts

fn extract_title(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);

    // Try <title> tag
    if let Some(el) = doc.select(&scraper::Selector::parse("title").unwrap()).next() {
        let title = el.text().collect::<String>().trim().to_string();
        if !title.is_empty() {
            return title;
        }
    }

    // Try <meta property="og:title">
    if let Ok(sel) = scraper::Selector::parse(r#"meta[property="og:title"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let title = content.trim().to_string();
                if !title.is_empty() {
                    return title;
                }
            }
        }
    }

    // Try first <h1>
    if let Some(el) = doc.select(&scraper::Selector::parse("h1").unwrap()).next() {
        let title = el.text().collect::<String>().trim().to_string();
        if !title.is_empty() {
            return title;
        }
    }

    "Untitled".to_string()
}

fn extract_main_content(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);

    // Priority content selectors (same strategy as Defuddle)
    let content_selectors = [
        "article",
        "main",
        "[role=main]",
        ".post-content",
        ".entry-content",
        ".article-content",
        ".answer",           // StackOverflow
        "#content",
        ".content",
        "#main-content",
        ".markdown-body",   // GitHub
        ".post",
    ];

    // Noise selectors to remove
    let noise_selectors = [
        "nav", "script", "style", "footer", "header", "noscript",
        "iframe", "svg", "form",
        ".sidebar", ".menu", ".nav", ".navigation",
        ".ad", ".ads", ".advertisement", ".adsbygoogle",
        ".cookie", ".cookie-banner", ".cookie-consent",
        ".popup", ".modal", ".overlay",
        ".social-share", ".share-buttons", ".social",
        ".comments", ".comment-section",
        ".related-posts", ".recommended",
        ".newsletter", ".subscribe",
        "[role=navigation]", "[role=banner]", "[role=contentinfo]",
        "[role=complementary]", "[aria-hidden=true]",
    ];

    // Try each content selector — use the first match
    for sel_str in &content_selectors {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            if let Some(el) = doc.select(&sel).next() {
                let mut content_html = el.html();

                // Strip noise from content
                let content_doc = scraper::Html::parse_fragment(&content_html);
                for noise_sel_str in &noise_selectors {
                    if let Ok(noise_sel) = scraper::Selector::parse(noise_sel_str) {
                        for noise_el in content_doc.select(&noise_sel) {
                            let noise_html = noise_el.html();
                            content_html = content_html.replace(&noise_html, "");
                        }
                    }
                }

                if content_html.len() > 100 {
                    return content_html;
                }
            }
        }
    }

    // Fallback: use <body>, stripping noise
    if let Some(body) = doc.select(&scraper::Selector::parse("body").unwrap()).next() {
        let mut body_html = body.html();
        for noise_sel_str in &noise_selectors {
            if let Ok(noise_sel) = scraper::Selector::parse(noise_sel_str) {
                for noise_el in doc.select(&noise_sel) {
                    let noise_html = noise_el.html();
                    body_html = body_html.replace(&noise_html, "");
                }
            }
        }
        return body_html;
    }

    // Last resort: return original HTML
    html.to_string()
}

// ===== Post-Processing (1:1 from MD-This-Page tabs/markdown.tsx:282-338) =====

fn post_process_markdown(md: &str, title: &str, url: &str, mode: &str) -> String {
    let mut result = md.to_string();

    match mode {
        "minimal" => {
            // Strip images: ![alt](url) and <img> tags (markdown.tsx:287-291)
            result = strip_images(&result);
            // Strip links: [text](url) → text, <a>text</a> → text (markdown.tsx:294-299)
            result = strip_links(&result);
        }
        "compact" => {
            // Strip images only
            result = strip_images(&result);
        }
        "full" | _ => {
            // Keep everything — images and links preserved
        }
    }

    // Always: clean up stray dashes/dots (markdown.tsx:332)
    // Remove lines containing only a dash or middle dot
    let mut cleaned_lines: Vec<&str> = Vec::new();
    for line in result.lines() {
        let trimmed = line.trim();
        if trimmed == "-" || trimmed == "·" || trimmed == "•" || trimmed == "—" {
            continue;
        }
        cleaned_lines.push(line);
    }
    result = cleaned_lines.join("\n");

    // Clean blank lines with spaces, collapse 3+ newlines → 2 (markdown.tsx:335-336)
    let mut prev_empty = 0;
    let mut final_lines: Vec<&str> = Vec::new();
    for line in result.lines() {
        if line.trim().is_empty() {
            prev_empty += 1;
            if prev_empty <= 2 {
                final_lines.push("");
            }
        } else {
            prev_empty = 0;
            final_lines.push(line);
        }
    }
    result = final_lines.join("\n").trim().to_string();

    // Build metadata header (markdown.tsx:302-317)
    let mut header = String::new();
    if !title.is_empty() && title != "Untitled" {
        header.push_str(&format!("**Title:** {}\n\n", title));
    }
    header.push_str(&format!("**Source:** {}\n\n---\n\n", url));

    // Page structure map (markdown.tsx:319-327)
    let page_map = generate_page_map(&result, title);
    if !page_map.is_empty() {
        header.push_str(&page_map);
        header.push_str("\n---\n\n");
    }

    format!("{}{}", header, result)
}

/// Strip markdown images and HTML img tags
fn strip_images(md: &str) -> String {
    let mut result = md.to_string();
    // Remove ![alt](url)
    while let Some(start) = result.find("![") {
        if let Some(mid) = result[start..].find("](") {
            if let Some(end) = result[start + mid..].find(')') {
                result = format!("{}{}", &result[..start], &result[start + mid + end + 1..]);
                continue;
            }
        }
        break;
    }
    // Remove <img ... > tags
    while let Some(start) = result.to_lowercase().find("<img") {
        if let Some(end) = result[start..].find('>') {
            result = format!("{}{}", &result[..start], &result[start + end + 1..]);
            continue;
        }
        break;
    }
    result
}

/// Strip markdown links but keep text, strip HTML anchor tags but keep text
fn strip_links(md: &str) -> String {
    let mut result = md.to_string();
    // Replace [text](url) → text
    loop {
        let Some(bracket_start) = result.find('[') else { break };
        // Make sure it's not an image ![
        if bracket_start > 0 && result.as_bytes()[bracket_start - 1] == b'!' {
            break;
        }
        let Some(bracket_end) = result[bracket_start..].find("](") else { break };
        let abs_bracket_end = bracket_start + bracket_end;
        let Some(paren_end) = result[abs_bracket_end + 2..].find(')') else { break };
        let abs_paren_end = abs_bracket_end + 2 + paren_end;
        let text = &result[bracket_start + 1..abs_bracket_end];
        result = format!("{}{}{}", &result[..bracket_start], text, &result[abs_paren_end + 1..]);
    }
    // Remove <a ...>text</a> → text
    while let Some(start) = result.to_lowercase().find("<a ") {
        if let Some(tag_end) = result[start..].find('>') {
            let content_start = start + tag_end + 1;
            if let Some(close) = result[content_start..].to_lowercase().find("</a>") {
                let text = &result[content_start..content_start + close];
                result = format!("{}{}{}", &result[..start], text, &result[content_start + close + 4..]);
                continue;
            }
        }
        break;
    }
    result
}

// ===== Page Structure Map (1:1 from markdown.tsx:183-251) =====

fn generate_page_map(markdown: &str, title: &str) -> String {
    let mut headings: Vec<(usize, String)> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level <= 6 {
                let text = trimmed[level..].trim().to_string();
                if !text.is_empty() {
                    headings.push((level, text));
                }
            }
        }
    }

    if headings.is_empty() {
        return String::new();
    }

    // Build tree structure and render with connectors
    let display_title = if title.is_empty() || title == "Untitled" {
        "Page structure map"
    } else {
        title
    };

    let mut map = format!("# Page Structure Map\n```text\n{}\n", display_title);

    struct HeadingNode {
        text: String,
        level: usize,
        children: Vec<HeadingNode>,
    }

    fn build_tree(headings: &[(usize, String)]) -> Vec<HeadingNode> {
        let mut roots: Vec<HeadingNode> = Vec::new();
        let mut stack: Vec<*mut HeadingNode> = Vec::new();

        for (level, text) in headings {
            let node = HeadingNode {
                text: text.clone(),
                level: *level,
                children: Vec::new(),
            };

            // Pop stack until we find a parent with lower level
            while let Some(&top) = stack.last() {
                let top_ref = unsafe { &*top };
                if top_ref.level >= *level {
                    stack.pop();
                } else {
                    break;
                }
            }

            if let Some(&parent) = stack.last() {
                let parent_ref = unsafe { &mut *parent };
                parent_ref.children.push(node);
                let last = parent_ref.children.last_mut().unwrap() as *mut HeadingNode;
                stack.push(last);
            } else {
                roots.push(node);
                let last = roots.last_mut().unwrap() as *mut HeadingNode;
                stack.push(last);
            }
        }

        roots
    }

    fn render_tree(map: &mut String, nodes: &[HeadingNode], prefix: &str) {
        for (i, node) in nodes.iter().enumerate() {
            let is_last = i == nodes.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            map.push_str(&format!("{}{}{}\n", prefix, connector, node.text));

            if !node.children.is_empty() {
                let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                render_tree(map, &node.children, &child_prefix);
            }
        }
    }

    let tree = build_tree(&headings);
    render_tree(&mut map, &tree, "");
    map.push_str("```");
    map
}
