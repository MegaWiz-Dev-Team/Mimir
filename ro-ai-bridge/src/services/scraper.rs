use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use anyhow::Result;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

pub struct ScraperService {
    browser: Browser,
    _handle: tokio::task::JoinHandle<()>,
}

impl ScraperService {
    pub async fn new() -> Result<Self> {
        let (browser, mut handler) = Browser::launch(BrowserConfig::builder()
            .with_head() // Run with head for debugging, switch to headless in prod
            .viewport(Some(chromiumoxide::handler::viewport::Viewport {
                width: 1920,
                height: 1080,
                device_scale_factor: Some(1.0),
                ..Default::default()
            }))
            .build()
            .map_err(|e| anyhow::anyhow!(e))?)
            .await?;

        let _handle = tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });

        Ok(Self { browser, _handle })
    }

    /// Scrape a generic URL with optional wait selector and scroll behavior
    pub async fn scrape_url(&self, url: &str, wait_selector: Option<&str>, scroll: bool) -> Result<String> {
        info!("Navigating to: {}", url);
        let page = self.browser.new_page(url).await?;
        
        // 1. Wait for Body or specific selector
        let selector = wait_selector.unwrap_or("body");
        info!("Waiting for selector: {}", selector);
        
        // Chromiumoxide 0.8+ usually employs find_element which waits implicitly or fails? 
        // Actually, creating a loop to wait is safer or using a specific wait function if available.
        // For now, let's try find_element which usually retries a bit or we sleep.
        // Better: use a manual wait loop for robustness if wait_for_selector isn't direct.
        let mut retries = 0;
        loop {
            if page.find_element(selector).await.is_ok() {
                break;
            }
            if retries > 10 {
                warn!("Timeout waiting for selector: {}", selector);
                break;
            }
            sleep(Duration::from_millis(500)).await;
            retries += 1;
        }

        // 2. Handle Login (Placeholder - if needed)
        // self.handle_login(&page).await?;

        // 3. Infinite Scroll Handling
        if scroll {
            info!("Starting scroll operation...");
            self.scroll_to_bottom(&page).await?;
        }
        
        let content = page.content().await?;
        page.close().await?;
        
        Ok(content)
    }

    #[allow(dead_code)]
    async fn handle_login(&self, _page: &Page) -> Result<()> {
        // Placeholder: If RO Landverse requires login for some news
        // 1. Check for login button
        // 2. Click login
        // 3. Fill user/pass
        // 4. Wait for redirect
        warn!("Login logic not implemented yet (not required for public news)");
        Ok(())
    }

    async fn scroll_to_bottom(&self, page: &Page) -> Result<()> {
        // Javascript to check scroll height
        let get_height_js = "document.body.scrollHeight";
        let scroll_js = "window.scrollTo(0, document.body.scrollHeight)";
        
        let mut previous_height = 0;
        let mut retries = 0;
        let max_retries = 5; // Stop if height doesn't change after 5 attempts
        
        loop {
            // Scroll down
            page.evaluate(scroll_js).await?;
            sleep(Duration::from_secs(2)).await; // Wait for content load
            
            // Check new height
            let new_height: i64 = page.evaluate(get_height_js).await?.into_value()?;
            
            if new_height == previous_height {
                retries += 1;
                if retries >= max_retries {
                    info!("Reached bottom of page or no new content loading.");
                    break;
                }
            } else {
                retries = 0; // Reset retries if we moved
                previous_height = new_height;
                info!("Scrolled to height: {}", new_height);
            }
        }
        
        Ok(())
    }
}
