#!/usr/bin/env python3
"""Playwright: Create Web Scraper sources for Prudential insurance products."""

import asyncio
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"
USERNAME = "zitadel-admin@asgard.localhost"
PASSWORD = "1qazXSW@"

# URLs from insurer_urls.json
SOURCES = [
    {
        "name": "Prudential - Health Products",
        "url": "https://prudential.co.th/en/products/health/",
        "type": "Web Scraper",
    },
    {
        "name": "Prudential - Life Products",
        "url": "https://prudential.co.th/en/products/life/",
        "type": "Web Scraper",
    },
    {
        "name": "Prudential Savings - Products",
        "url": "https://prudential.co.th/en/products/savings/",
        "type": "Web Scraper",
    },
]

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=False,
            args=["--ignore-certificate-errors"]
        )
        context = await browser.new_context(ignore_https_errors=True)
        page = await context.new_page()

        print("="*70)
        print("🌐 MIMIR WEB SCRAPER SOURCE CREATION")
        print("="*70)

        # Step 1: Login
        print("\n1️⃣ Logging in...")
        await page.goto(MIMIR_URL, wait_until="domcontentloaded")
        await page.wait_for_timeout(2000)

        # SSO Login
        username_input = await page.query_selector('input[type="text"], input[name*="login_name"]')
        if username_input:
            await username_input.fill(USERNAME)
            await page.wait_for_timeout(500)

            next_btn = await page.query_selector('button[type="submit"], button:has-text("Next")')
            if next_btn:
                await next_btn.click()
                await page.wait_for_timeout(3000)

            password_input = await page.query_selector('input[type="password"]')
            if password_input:
                await password_input.fill(PASSWORD)
                await page.wait_for_timeout(500)

                login_btn = await page.query_selector('button[type="submit"], button:has-text("Next")')
                if login_btn:
                    await login_btn.click()
                    await page.wait_for_timeout(3000)

        print("   ✅ Logged in")
        await page.screenshot(path="/tmp/web_1_logged_in.png")

        # Step 2: Navigate to asgard_insurance Data Integration
        print("\n2️⃣ Navigating to asgard_insurance Data Integration...")
        await page.goto("https://mimir.asgard.internal/insurance/data", wait_until="domcontentloaded")
        await page.wait_for_timeout(2000)
        print("   ✅ At Data page (asgard_insurance tenant)")

        # Step 3: Click on Sources tab
        sources_tab = await page.query_selector('button:has-text("Sources"), [role="tab"]:has-text("Sources")')
        if sources_tab:
            await sources_tab.click()
            await page.wait_for_timeout(1000)
            print("   ✅ In Sources tab")

        await page.screenshot(path="/tmp/web_2_sources_tab.png")

        # Step 4: For each source, click Add Source and select Web Scraper
        for i, source in enumerate(SOURCES, 1):
            print(f"\n3️⃣ Creating Source {i}: {source['name']}")

            # Click Add Source button
            add_btn = await page.query_selector('button:has-text("Add Source")')
            if add_btn:
                await add_btn.click()
                await page.wait_for_timeout(2000)
                print(f"   ✅ Clicked Add Source")

                await page.screenshot(path=f"/tmp/web_3_{i}_add_source_menu.png")

                # Click Web Scraper option
                web_scraper_btn = await page.query_selector('button:has-text("Web Scraper"), [role="button"]:has-text("Web Scraper")')
                if web_scraper_btn:
                    await web_scraper_btn.click()
                    await page.wait_for_timeout(2000)
                    print(f"   ✅ Clicked Web Scraper")

                    await page.screenshot(path=f"/tmp/web_4_{i}_scraper_form.png")

                    # Now find and fill the form fields
                    print(f"   🔍 Looking for form fields...")

                    # Look for URL input field
                    url_input = await page.query_selector('input[placeholder*="URL"], input[placeholder*="url"], input[type="url"]')
                    if url_input:
                        await url_input.fill(source['url'])
                        print(f"   ✅ Filled URL: {source['url']}")
                    else:
                        print(f"   ⚠️ URL input not found")
                        # Try to find by label
                        inputs = await page.query_selector_all("input[type='text'], input[type='url']")
                        print(f"      Found {len(inputs)} text inputs")

                    await page.wait_for_timeout(500)

                    # Look for name/source name input
                    name_input = await page.query_selector('input[placeholder*="name"], input[placeholder*="Name"]')
                    if name_input:
                        await name_input.fill(source['name'])
                        print(f"   ✅ Filled name: {source['name']}")
                    else:
                        # Try second text input if first is URL
                        inputs = await page.query_selector_all("input[type='text']")
                        if len(inputs) > 1:
                            await inputs[1].fill(source['name'])
                            print(f"   ✅ Filled name (2nd input): {source['name']}")

                    await page.wait_for_timeout(500)

                    # Look for Save/Create/Submit button
                    save_btn = await page.query_selector('button:has-text("Save"), button:has-text("Create"), button:has-text("Run"), button:has-text("Submit")')
                    if save_btn:
                        await save_btn.click()
                        await page.wait_for_timeout(3000)
                        print(f"   ✅ Clicked Save/Create")
                    else:
                        print(f"   ⚠️ Save button not found")
                        buttons = await page.query_selector_all("button")
                        print(f"      Available buttons:")
                        for btn in buttons[-5:]:  # Last 5 buttons
                            text = await btn.text_content()
                            if text.strip():
                                print(f"        - {text.strip()}")

                    await page.screenshot(path=f"/tmp/web_5_{i}_created.png")

                else:
                    print(f"   ⚠️ Web Scraper button not found")

            else:
                print(f"   ⚠️ Add Source button not found")

        # Step 5: Final overview
        print("\n" + "="*70)
        await page.screenshot(path="/tmp/web_6_final_overview.png")
        print("✅ Web sources created!")
        print("="*70)

        print("\n📸 Screenshots:")
        for i in range(1, 7):
            print(f"  - /tmp/web_{i}_*.png")

        print("\n💡 Browser will stay open for 30 seconds")
        await page.wait_for_timeout(30000)

        await browser.close()
        print("\n✨ Done!\n")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:
        print(f"\n❌ Error: {e}")
