use anyhow::Result;
use dotenvy::dotenv;
use mimir_core_ai::services::scraper::ScraperService;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let output_dir = "data/wiki";
    if !Path::new(output_dir).exists() {
        fs::create_dir_all(output_dir).await?;
    }

    let base_url = "https://maxion-1.gitbook.io/ragnarok-landverse-th/";

    // 2. Initialize Scraper
    info!("🚀 Initializing Browser for Crawling...");
    let scraper_service = ScraperService::new().await?;

    // 3. Fetch Sitemap (Home Page contains the Sidebar)
    info!("🔎 Fetching Sitemap from Home: {}", base_url);
    // Wait for the sidebar to load
    let home_html = scraper_service
        .scrape_url(base_url, Some("nav"), true)
        .await?;

    // 4. Extract Links
    let document = Html::parse_document(&home_html);
    let selector = Selector::parse("a").unwrap();

    let mut links = HashSet::new();

    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("href") {
            // Filter for internal wiki links
            if href.starts_with("/ragnarok-landverse-th/") || href.starts_with(base_url) {
                // Clean up anchors (e.g. #some-header)
                let clean_href = href.split('#').next().unwrap_or(href);

                // Normalize to full URL
                let full_url = if clean_href.starts_with("http") {
                    clean_href.to_string()
                } else {
                    format!("https://maxion-1.gitbook.io{}", clean_href)
                };

                links.insert(full_url);
            }
        }
    }

    info!("📊 Found {} unique pages to scrape.", links.len());

    // Convert to Vec for deterministic order
    let mut links_vec: Vec<String> = links.into_iter().collect();
    links_vec.sort();

    // 5. Crawler Loop
    for (i, url) in links_vec.iter().enumerate() {
        info!("[{}/{}] 🌍 Scraping: {}", i + 1, links_vec.len(), url);

        // Wait for main content or at least body
        match scraper_service.scrape_url(url, Some("main"), true).await {
            Ok(html) => {
                info!("   ✅ Fetched. Cleaning content...");

                // Parse and Clean
                let document = Html::parse_document(&html);

                // Try to find <main>, fallback to <body>
                let main_selector = Selector::parse("main").unwrap();
                let body_selector = Selector::parse("body").unwrap();

                let root_element = document
                    .select(&main_selector)
                    .next()
                    .or_else(|| document.select(&body_selector).next());

                if let Some(root) = root_element {
                    // Extract inner HTML
                    let inner_html = root.inner_html();

                    // Sanitize
                    let clean_html = clean_html_content(&inner_html);

                    // Convert to Markdown
                    let markdown = html2md::parse_html(&clean_html);

                    // Extract Title (H1)
                    let title_selector = Selector::parse("h1").unwrap();
                    let title = document
                        .select(&title_selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(""))
                        .unwrap_or_else(|| "Untitled".to_string());

                    // Add YAML Frontmatter
                    let frontmatter = format!(
                        "---\ntitle: \"{}\"\nurl: \"{}\"\n---\n\n",
                        title.replace("\"", "\\\""),
                        url
                    );
                    let final_content = format!("{}{}", frontmatter, markdown);

                    // Generate Filename
                    let clean_base = "https://maxion-1.gitbook.io/ragnarok-landverse-th/";
                    let relative_path = url.replace(clean_base, "");
                    let safe_name = relative_path.replace("/", "_").replace("-", "_");
                    let filename = if safe_name.is_empty() {
                        "Home.md".to_string()
                    } else {
                        format!("{}.md", safe_name)
                    };
                    let filename = filename.trim_start_matches('_').to_string();
                    let path = format!("{}/{}", output_dir, filename);

                    fs::write(&path, final_content).await?;
                    info!("   💾 Saved to {}", path);
                } else {
                    warn!("   ⚠️ No content found for {}", url);
                }
            }
            Err(e) => error!("   ❌ Failed to scrape {}: {}", url, e),
        }
    }

    info!("✨ Wiki sync completed! All docs downloaded.");
    Ok(())
}

fn clean_html_content(html: &str) -> String {
    let mut text = html.to_string();

    // Regex patterns to remove
    let patterns = [
        r"(?s)<script.*?>.*?</script>",     // Scripts
        r"(?s)<style.*?>.*?</style>",       // Styles
        r"(?s)<nav.*?>.*?</nav>",           // Navigation
        r"(?s)<footer.*?>.*?</footer>",     // Footer
        r"(?s)<!--.*?-->",                  // Comments
        r"(?s)<noscript.*?>.*?</noscript>", // Noscript
        r"(?s)<iframe.*?>.*?</iframe>",     // Iframes
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            text = re.replace_all(&text, "").to_string();
        }
    }

    // Additional cleanup for common Gitbook artifacts via regex if possible
    // or just rely on html2md ignoring attributes.
    // However, removing empty structured tags might be good?
    // Let's stick to the basics first.

    text
}
