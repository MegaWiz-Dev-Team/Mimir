//! Link discovery service: URL preview (OG metadata) and same-domain link extraction.
//!
//! Uses reqwest + scraper crate for lightweight HTML parsing.
//! Does NOT use headless browser — that's in scraper.rs for JS-heavy pages.

use anyhow::{Result, bail};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::info;
use url::Url;

// ─── Types ─────────────────────────────────────────────────────────────────────

/// OG metadata preview for a URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlPreview {
    pub url: String,
    pub domain: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub favicon: Option<String>,
}

/// A discovered link from a page.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredLink {
    pub url: String,
    pub text: String,
}

// ─── URL Preview ───────────────────────────────────────────────────────────────

/// Fetch a URL and extract OG metadata for preview.
pub async fn fetch_url_preview(url: &str) -> Result<UrlPreview> {
    let parsed = Url::parse(url).map_err(|e| anyhow::anyhow!("Invalid URL '{}': {}", url, e))?;

    let domain = parsed.host_str().unwrap_or("unknown").to_string();

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("MimirBot/1.0")
        .build()?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch '{}': {}", url, e))?;

    if !response.status().is_success() {
        bail!("HTTP {} for '{}'", response.status(), url);
    }

    let html = response.text().await?;
    let preview = parse_og_metadata(&html, url, &domain);

    info!("URL preview for {}: title={:?}", url, preview.title);
    Ok(preview)
}

/// Parse OG metadata from raw HTML.
pub fn parse_og_metadata(html: &str, url: &str, domain: &str) -> UrlPreview {
    let doc = Html::parse_document(html);

    let title = extract_meta(&doc, "og:title")
        .or_else(|| extract_meta(&doc, "twitter:title"))
        .or_else(|| extract_title_tag(&doc));

    let description = extract_meta(&doc, "og:description")
        .or_else(|| extract_meta(&doc, "twitter:description"))
        .or_else(|| extract_meta_name(&doc, "description"));

    let image = extract_meta(&doc, "og:image").or_else(|| extract_meta(&doc, "twitter:image"));

    let favicon = extract_favicon(&doc, url);

    UrlPreview {
        url: url.to_string(),
        domain: domain.to_string(),
        title,
        description,
        image,
        favicon,
    }
}

// ─── Link Discovery ────────────────────────────────────────────────────────────

/// Extract links from HTML, filtered to same domain, deduplicated.
///
/// - `base_url`: the URL the HTML was fetched from (for resolving relative links)
/// - `max_links`: maximum number of links to return
pub fn discover_links(html: &str, base_url: &str, max_links: usize) -> Vec<DiscoveredLink> {
    let doc = Html::parse_document(html);
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return vec![],
    };
    let base_domain = base.host_str().unwrap_or("").to_string();

    let link_sel = Selector::parse("a[href]").unwrap();
    let mut seen = HashSet::new();
    let mut links = Vec::new();

    for el in doc.select(&link_sel) {
        if links.len() >= max_links {
            break;
        }

        let href = match el.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // Resolve relative URLs
        let resolved = match base.join(href) {
            Ok(u) => u,
            Err(_) => continue,
        };

        // Filter: same domain only, http/https only
        let scheme = resolved.scheme();
        if scheme != "http" && scheme != "https" {
            continue;
        }

        let link_domain = resolved.host_str().unwrap_or("");
        if link_domain != base_domain {
            continue;
        }

        // Normalize: strip fragment
        let mut normalized = resolved.clone();
        normalized.set_fragment(None);
        let url_str = normalized.to_string();

        // Deduplicate
        if seen.contains(&url_str) {
            continue;
        }
        seen.insert(url_str.clone());

        // Skip non-page resources
        let path = resolved.path().to_lowercase();
        if path.ends_with(".jpg")
            || path.ends_with(".png")
            || path.ends_with(".gif")
            || path.ends_with(".css")
            || path.ends_with(".js")
            || path.ends_with(".pdf")
        {
            continue;
        }

        let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();

        links.push(DiscoveredLink {
            url: url_str,
            text: if text.is_empty() {
                href.to_string()
            } else {
                text
            },
        });
    }

    links
}

// ─── Content Hash ──────────────────────────────────────────────────────────────

/// Compute SHA-256 hash of content for change detection.
pub fn compute_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ─── HTML Parsing Helpers ──────────────────────────────────────────────────────

fn extract_meta(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!("meta[property=\"{}\"]", property)).ok()?;
    doc.select(&sel)
        .next()?
        .value()
        .attr("content")
        .map(|s| s.to_string())
}

fn extract_meta_name(doc: &Html, name: &str) -> Option<String> {
    let sel = Selector::parse(&format!("meta[name=\"{}\"]", name)).ok()?;
    doc.select(&sel)
        .next()?
        .value()
        .attr("content")
        .map(|s| s.to_string())
}

fn extract_title_tag(doc: &Html) -> Option<String> {
    let sel = Selector::parse("title").ok()?;
    let el = doc.select(&sel).next()?;
    let text = el.text().collect::<Vec<_>>().join("");
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn extract_favicon(doc: &Html, base_url: &str) -> Option<String> {
    // Try <link rel="icon"> or <link rel="shortcut icon">
    for sel_str in &["link[rel=\"icon\"]", "link[rel=\"shortcut icon\"]", "link[rel=\"apple-touch-icon\"]"] {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = doc.select(&sel).next() {
                if let Some(href) = el.value().attr("href") {
                    // Resolve relative favicon URL
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(resolved) = base.join(href) {
                            return Some(resolved.to_string());
                        }
                    }
                    return Some(href.to_string());
                }
            }
        }
    }
    // Default: /favicon.ico
    if let Ok(base) = Url::parse(base_url) {
        if let Ok(fav) = base.join("/favicon.ico") {
            return Some(fav.to_string());
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test Page Title</title>
        <meta property="og:title" content="OG Title Here">
        <meta property="og:description" content="OG Description">
        <meta property="og:image" content="https://example.com/image.png">
        <meta name="description" content="Meta Description Fallback">
        <link rel="icon" href="/favicon.png">
    </head>
    <body>
        <a href="/about">About Us</a>
        <a href="/products">Products</a>
        <a href="https://example.com/contact">Contact</a>
        <a href="https://other-domain.com/external">External Link</a>
        <a href="/about">About Us Duplicate</a>
        <a href="/image.jpg">Image Link</a>
        <a href="#section">Fragment Only</a>
        <a href="mailto:test@example.com">Email</a>
    </body>
    </html>
    "##;

    #[test]
    fn test_parse_og_metadata() {
        let preview = parse_og_metadata(SAMPLE_HTML, "https://example.com", "example.com");
        assert_eq!(preview.title.as_deref(), Some("OG Title Here"));
        assert_eq!(preview.description.as_deref(), Some("OG Description"));
        assert_eq!(
            preview.image.as_deref(),
            Some("https://example.com/image.png")
        );
        assert_eq!(preview.domain, "example.com");
    }

    #[test]
    fn test_parse_og_fallback_to_title() {
        let html = r##"
        <html><head><title>Fallback Title</title></head><body></body></html>
        "##;
        let preview = parse_og_metadata(html, "https://test.com", "test.com");
        assert_eq!(preview.title.as_deref(), Some("Fallback Title"));
        assert!(preview.description.is_none());
    }

    #[test]
    fn test_parse_og_meta_name_description() {
        let html = r##"
        <html><head>
            <meta name="description" content="Name-based description">
        </head><body></body></html>
        "##;
        let preview = parse_og_metadata(html, "https://test.com", "test.com");
        assert_eq!(
            preview.description.as_deref(),
            Some("Name-based description")
        );
    }

    #[test]
    fn test_discover_links_same_domain() {
        let links = discover_links(SAMPLE_HTML, "https://example.com", 100);
        // Should include /about, /products, /contact (same domain)
        // Should exclude external, image, mailto, fragment
        let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
        assert!(
            urls.contains(&"https://example.com/about"),
            "Should contain /about"
        );
        assert!(
            urls.contains(&"https://example.com/products"),
            "Should contain /products"
        );
        assert!(
            urls.contains(&"https://example.com/contact"),
            "Should contain /contact"
        );
        assert!(
            !urls.iter().any(|u| u.contains("other-domain")),
            "Should not contain external links"
        );
        assert!(
            !urls.iter().any(|u| u.contains("image.jpg")),
            "Should not contain image links"
        );
    }

    #[test]
    fn test_discover_links_dedup() {
        let links = discover_links(SAMPLE_HTML, "https://example.com", 100);
        let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
        let unique: HashSet<&&str> = urls.iter().collect();
        assert_eq!(urls.len(), unique.len(), "Should have no duplicates");
    }

    #[test]
    fn test_discover_links_max_limit() {
        let links = discover_links(SAMPLE_HTML, "https://example.com", 2);
        assert!(links.len() <= 2, "Should respect max_links limit");
    }

    #[test]
    fn test_compute_content_hash() {
        let hash1 = compute_content_hash("hello world");
        let hash2 = compute_content_hash("hello world");
        let hash3 = compute_content_hash("different content");
        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_ne!(
            hash1, hash3,
            "Different content should produce different hash"
        );
        assert_eq!(hash1.len(), 64, "SHA-256 hex should be 64 chars");
    }

    #[test]
    fn test_favicon_extraction() {
        let preview = parse_og_metadata(SAMPLE_HTML, "https://example.com", "example.com");
        assert_eq!(
            preview.favicon.as_deref(),
            Some("https://example.com/favicon.png")
        );
    }

    #[test]
    fn test_favicon_default_fallback() {
        let html = "<html><head></head><body></body></html>";
        let preview = parse_og_metadata(html, "https://example.com", "example.com");
        assert_eq!(
            preview.favicon.as_deref(),
            Some("https://example.com/favicon.ico")
        );
    }
}
