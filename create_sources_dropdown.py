#!/usr/bin/env python3
"""Playwright: Create sources using tenant dropdown."""

import asyncio
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"
USERNAME = "zitadel-admin@asgard.localhost"
PASSWORD = "1qazXSW@"

SOURCES = [
    {
        "name": "Prudential - Health Products",
        "url": "https://prudential.co.th/en/products/health/",
    },
    {
        "name": "Prudential - Life Products",
        "url": "https://prudential.co.th/en/products/life/",
    },
    {
        "name": "Prudential Savings - Products",
        "url": "https://prudential.co.th/en/products/savings/",
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
        print("🎯 MIMIR WEB SOURCES - USING TENANT DROPDOWN")
        print("="*70)

        # Step 1: Login
        print("\n1️⃣ Logging in...")
        await page.goto(MIMIR_URL, wait_until="domcontentloaded")
        await page.wait_for_timeout(2000)

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
        await page.screenshot(path="/tmp/s1_logged_in.png")

        # Step 2: Change Tenant using dropdown
        print("\n2️⃣ Changing tenant via dropdown...")
        tenant_select = await page.query_selector('select, select[name*="tenant"]')

        if tenant_select:
            print("   ✅ Found SELECT element")

            # Get all options
            options = await tenant_select.query_selector_all("option")
            print(f"   📋 Available options ({len(options)}):")
            for opt in options:
                text = await opt.text_content()
                value = await opt.get_attribute("value")
                print(f"      - {text.strip()} (value: {value})")

            # Use selectOption to change value
            await tenant_select.select_option("asgard_insurance")
            print(f"   ✅ Selected: asgard_insurance (Insurance Product Platform)")
            await page.wait_for_timeout(2000)

        else:
            print("   ⚠️  SELECT element not found, trying dropdown button...")

            # Try clicking a dropdown button
            dropdown_btn = await page.query_selector('[role="combobox"], button:has-text("Mega Care"), button:has-text("Select")')
            if dropdown_btn:
                await dropdown_btn.click()
                await page.wait_for_timeout(1500)
                print("   ✅ Clicked dropdown button")

                await page.screenshot(path="/tmp/s2_dropdown_open.png")

                # Find and click asgard_insurance option
                all_elements = await page.query_selector_all("[role='option'], li, div, button")
                print(f"   🔍 Checking {len(all_elements)} elements...")

                for elem in all_elements:
                    text = await elem.text_content()
                    if text and ("asgard" in text.lower() or "insurance" in text.lower()):
                        print(f"   ✅ Found option: {text.strip()}")
                        await elem.click()
                        await page.wait_for_timeout(2000)
                        break

        await page.screenshot(path="/tmp/s3_tenant_changed.png")
        print("   ✅ Tenant changed")

        # Step 3: Click Data menu
        print("\n3️⃣ Going to Data page...")
        data_link = await page.query_selector('a:has-text("Data"), button:has-text("Data"), [href*="data"]')
        if data_link:
            await data_link.click()
            await page.wait_for_timeout(2000)
            print("   ✅ Clicked Data")
        else:
            print("   ⚠️  Data link not found")

        await page.screenshot(path="/tmp/s4_data_page.png")

        # Step 4: Click Sources tab
        print("\n4️⃣ Going to Sources tab...")
        sources_tab = await page.query_selector('button:has-text("Sources")')
        if sources_tab:
            await sources_tab.click()
            await page.wait_for_timeout(1500)
            print("   ✅ In Sources tab")

        await page.screenshot(path="/tmp/s5_sources_tab.png")

        # Step 5: Create sources
        print("\n5️⃣ Creating Web Scraper sources...")

        # Scroll down to see Add Source button
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await page.wait_for_timeout(1000)

        # Debug: List all buttons
        all_buttons = await page.query_selector_all("button")
        print(f"\n   📋 Found {len(all_buttons)} buttons on page:")
        for j, btn in enumerate(all_buttons[-10:]):  # Last 10 buttons
            text = await btn.text_content()
            if text.strip():
                print(f"      {j}. {text.strip()}")

        for i, source in enumerate(SOURCES, 1):
            print(f"\n   Source {i}: {source['name']}")

            # Click Add Source - try direct locator
            try:
                await page.locator('button:has-text("Add Source")').first.click(timeout=5000)
                print(f"      ✅ Clicked Add Source (locator)")
                await page.wait_for_timeout(2000)
            except:
                # Fallback: try clicking by text using locator
                try:
                    btn_handle = await page.locator("text=Add Source").first.element_handle()
                    if btn_handle:
                        await btn_handle.click()
                        print(f"      ✅ Clicked Add Source (text locator)")
                        await page.wait_for_timeout(2000)
                    else:
                        print(f"      ⚠️  Add Source button not found")
                except Exception as e:
                    print(f"      ⚠️  Error clicking Add Source: {e}")
                    continue

            # If we got here, button was clicked, so continue with form
            add_btn = True

            if add_btn:

                await page.screenshot(path=f"/tmp/s6_{i}_add_source.png")

                # Click Web Scraper
                web_scraper = await page.query_selector('button:has-text("Web Scraper")')
                if web_scraper:
                    await web_scraper.click()
                    await page.wait_for_timeout(2000)
                    print(f"      ✅ Selected Web Scraper")

                    await page.screenshot(path=f"/tmp/s7_{i}_form.png")

                    # Fill URL
                    url_input = await page.query_selector('input[type="url"], input[placeholder*="URL"], input[placeholder*="url"]')
                    if not url_input:
                        # Try all text inputs
                        inputs = await page.query_selector_all("input[type='text']")
                        if inputs:
                            url_input = inputs[0]

                    if url_input:
                        await url_input.fill(source['url'])
                        print(f"      ✅ Filled URL: {source['url']}")
                    else:
                        print(f"      ⚠️  URL input not found")

                    await page.wait_for_timeout(500)

                    # Fill Name (if there's another input)
                    inputs = await page.query_selector_all("input[type='text']")
                    if len(inputs) > 1:
                        await inputs[1].fill(source['name'])
                        print(f"      ✅ Filled name: {source['name']}")

                    await page.wait_for_timeout(500)

                    # Save
                    save_btn = await page.query_selector('button:has-text("Save"), button:has-text("Create"), button:has-text("Run")')
                    if save_btn:
                        await save_btn.click()
                        await page.wait_for_timeout(3000)
                        print(f"      ✅ Saved source")
                    else:
                        print(f"      ⚠️  Save button not found")

                    await page.screenshot(path=f"/tmp/s8_{i}_created.png")

                else:
                    print(f"      ⚠️  Web Scraper button not found")

            else:
                print(f"      ⚠️  Add Source button not found")

        # Final
        print("\n" + "="*70)
        print("✅ Done!")
        print("="*70)
        print("\n📸 Screenshots saved to /tmp/s*.png")
        print("\n💡 Browser staying open for 30s...")

        await page.wait_for_timeout(30000)
        await browser.close()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
