#!/usr/bin/env python3
"""Playwright script to create insurance sources in Mimir UI."""

import asyncio
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"  # Mimir Dashboard

async def create_sources():
    """Create sources for S2 insurance products."""

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=False)
        page = await browser.new_page()

        print(f"📱 Opening Mimir at {MIMIR_URL}")
        await page.goto(f"{MIMIR_URL}/insurance", wait_until="networkidle")

        # Wait for page to load
        await page.wait_for_timeout(2000)

        # Take screenshot of current state
        await page.screenshot(path="/tmp/mimir_before.png")
        print("📸 Screenshot 1: Current state saved")

        # Navigate to Data → Sources
        print("\n🔍 Looking for Data menu...")
        try:
            await page.click("text=Data")
            await page.wait_for_timeout(1000)
            print("✅ Clicked Data menu")
        except Exception as e:
            print(f"⚠️  Could not find Data menu: {e}")

        # Look for Sources tab
        try:
            await page.click("text=Sources")
            await page.wait_for_timeout(1000)
            print("✅ Clicked Sources tab")
        except Exception as e:
            print(f"⚠️  Could not find Sources tab: {e}")

        # Take screenshot
        await page.screenshot(path="/tmp/mimir_sources.png")
        print("📸 Screenshot 2: Sources page saved")

        # Look for Add Source button
        try:
            add_btn = await page.query_selector("text=Add Source")
            if add_btn:
                print("✅ Found 'Add Source' button")
                await add_btn.click()
                await page.wait_for_timeout(2000)
                print("✅ Clicked Add Source")
            else:
                print("⚠️  No 'Add Source' button found")
        except Exception as e:
            print(f"⚠️  Could not click Add Source: {e}")

        # Take screenshot of form
        await page.screenshot(path="/tmp/mimir_form.png")
        print("📸 Screenshot 3: Add Source form saved")

        # Print page HTML for debugging
        content = await page.content()
        print(f"\n📄 Page HTML length: {len(content)} chars")
        print("🔎 Looking for form fields...")

        # Print all visible text
        text_content = await page.evaluate("() => document.body.innerText")
        print(f"\n--- Visible Text ---\n{text_content[:1000]}\n")

        # List all inputs/selects
        inputs = await page.query_selector_all("input, select, button")
        print(f"\n📋 Found {len(inputs)} input/select/button elements:")
        for i, inp in enumerate(inputs[:20]):
            tag = await inp.evaluate("el => el.tagName")
            placeholder = await inp.evaluate("el => el.placeholder || ''")
            value = await inp.evaluate("el => el.value || ''")
            text = await inp.evaluate("el => el.innerText || ''")
            print(f"  {i}. <{tag}> placeholder='{placeholder}' value='{value}' text='{text}'")

        await page.screenshot(path="/tmp/mimir_debug.png")
        print("\n📸 Screenshot 4: Debug page saved")

        print("\n" + "="*60)
        print("Screenshots saved to:")
        print("  - /tmp/mimir_before.png")
        print("  - /tmp/mimir_sources.png")
        print("  - /tmp/mimir_form.png")
        print("  - /tmp/mimir_debug.png")
        print("="*60)

        await browser.close()

if __name__ == "__main__":
    asyncio.run(create_sources())
