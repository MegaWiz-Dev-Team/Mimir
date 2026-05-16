#!/usr/bin/env python3
"""Playwright: Create insurance sources (manual login)."""

import asyncio
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=False,
            args=["--ignore-certificate-errors"]
        )
        context = await browser.new_context(ignore_https_errors=True)
        page = await context.new_page()

        print("="*70)
        print("🌐 Playwright Browser Opening")
        print("="*70)
        print(f"\n📍 URL: {MIMIR_URL}")
        print("\n⚠️  MANUAL LOGIN REQUIRED:")
        print("  1. ใส่ username และ password เอง")
        print("  2. หลังจาก login เสร็จ ให้ปล่อยไว้")
        print("  3. Script จะรอจนกว่า URL จะเป็น /insurance")
        print("\n💡 Waiting for login...")
        print("="*70 + "\n")

        # Open Mimir
        await page.goto(MIMIR_URL, wait_until="domcontentloaded")

        # Wait for login to complete (URL should change to /insurance)
        # Timeout after 5 minutes
        try:
            await page.wait_for_url("**/insurance**", timeout=300000)
            print("\n✅ Login successful!")
            await page.wait_for_timeout(2000)
        except Exception as e:
            print(f"\n⚠️  Timeout waiting for login: {e}")
            print("Browser will stay open - คุณสามารถ login เอง")
            await page.wait_for_timeout(60000)

        print("\n" + "="*70)
        print("📋 Now Creating Sources...")
        print("="*70 + "\n")

        current_url = page.url
        print(f"📍 Current URL: {current_url}\n")

        # Click Data menu
        try:
            print("🔍 Looking for Data menu...")
            data_menu = await page.query_selector('a:has-text("Data"), button:has-text("Data"), [href*="data"]')

            if not data_menu:
                # Try to find by text content
                elements = await page.query_selector_all("*")
                for elem in elements:
                    text = await elem.text_content()
                    if text and "Data" in text:
                        data_menu = elem
                        break

            if data_menu:
                await data_menu.click()
                await page.wait_for_timeout(1500)
                print("✅ Clicked Data menu\n")
            else:
                print("⚠️  Could not find Data menu")
                # Print all navigation items
                nav_items = await page.query_selector_all("nav a, nav button")
                print(f"Found {len(nav_items)} nav items:")
                for i, item in enumerate(nav_items[:10]):
                    text = await item.text_content()
                    print(f"  {i}. {text.strip()}")

        except Exception as e:
            print(f"❌ Error clicking Data: {e}")

        # Take screenshot of page
        await page.screenshot(path="/tmp/mimir_data_page.png")
        print("📸 Screenshot saved: /tmp/mimir_data_page.png")

        # Click Sources tab
        try:
            print("\n🔍 Looking for Sources tab...")
            sources_tab = await page.query_selector('a:has-text("Sources"), button:has-text("Sources"), [href*="source"]')

            if sources_tab:
                await sources_tab.click()
                await page.wait_for_timeout(1500)
                print("✅ Clicked Sources tab\n")
            else:
                print("⚠️  Could not find Sources tab")

        except Exception as e:
            print(f"⚠️  Error with Sources: {e}")

        await page.screenshot(path="/tmp/mimir_sources_tab.png")
        print("📸 Screenshot saved: /tmp/mimir_sources_tab.png")

        # Look for Add Source button
        try:
            print("\n🔍 Looking for 'Add Source' button...")
            add_btn = await page.query_selector('button:has-text("Add Source"), a:has-text("Add Source")')

            if add_btn:
                print("✅ Found 'Add Source' button")
                await add_btn.click()
                await page.wait_for_timeout(2000)
                print("✅ Clicked Add Source\n")

                await page.screenshot(path="/tmp/mimir_add_source_modal.png")
                print("📸 Screenshot saved: /tmp/mimir_add_source_modal.png")

                # Now let user fill the form manually
                print("="*70)
                print("📝 ADD SOURCE FORM OPENED")
                print("="*70)
                print("""
ใส่ข้อมูลต่อไปนี้สำหรับ Source แรก:

  Source Name:     Prudential Insurance Products
  Type:            Qdrant (Vector DB)
  Collection:      insurance_products_001
  Tenant:          asgard_insurance

แล้ว Save

กด Enter เมื่อเสร็จ...
                """)

                input("⏳ Waiting... (press Enter when source created)")

                # Save screenshot after creation
                await page.screenshot(path="/tmp/mimir_source_created.png")
                print("📸 Screenshot saved: /tmp/mimir_source_created.png\n")

            else:
                print("⚠️  Could not find Add Source button")
                # List all buttons
                buttons = await page.query_selector_all("button")
                print(f"\nFound {len(buttons)} buttons:")
                for i, btn in enumerate(buttons[:15]):
                    text = await btn.text_content()
                    if text.strip():
                        print(f"  {i}. {text.strip()}")

        except Exception as e:
            print(f"❌ Error with Add Source: {e}")

        print("\n" + "="*70)
        print("✅ Source creation workflow ready!")
        print("="*70)
        print("\nBrowser will remain open. Close it when done.")
        print("Press Ctrl+C to exit.\n")

        # Keep browser open
        await page.wait_for_timeout(600000)

        await browser.close()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Browser closed.")
