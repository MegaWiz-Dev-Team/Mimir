#!/usr/bin/env python3
"""Playwright: Auto-create insurance sources with full login automation."""

import asyncio
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"
USERNAME = "zitadel-admin@asgard.localhost"
PASSWORD = "1qazXSW@"

SOURCES = [
    {
        "name": "Prudential Insurance Products",
        "collection": "insurance_products_001",
        "type": "Qdrant",
    },
    {
        "name": "Prudential Savings Products",
        "collection": "insurance_products_002",
        "type": "Qdrant",
    }
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
        print("🚀 MIMIR AUTO SOURCE CREATION")
        print("="*70)
        print(f"\n📍 URL: {MIMIR_URL}")
        print(f"👤 User: {USERNAME}")
        print(f"🔐 Password: {'*' * len(PASSWORD)}\n")

        # Step 1: Navigate to Mimir
        print("Step 1️⃣ : Navigating to Mimir...")
        await page.goto(MIMIR_URL, wait_until="domcontentloaded")
        await page.wait_for_timeout(2000)

        current_url = page.url
        print(f"  Current URL: {current_url}")

        # Step 2: SSO Login
        if "login" in current_url.lower() or "sso" in current_url.lower():
            print("\nStep 2️⃣ : SSO Login Required")

            # Enter username
            print("  ✓ Entering username...")
            username_input = await page.query_selector('input[type="text"], input[placeholder*="username"], input[name*="login_name"]')
            if username_input:
                await username_input.fill(USERNAME)
                await page.wait_for_timeout(500)
                print(f"    ✅ Entered: {USERNAME}")
            else:
                print("    ⚠️  Username field not found")
                inputs = await page.query_selector_all("input")
                for i, inp in enumerate(inputs):
                    print(f"      Input {i}: {await inp.get_attribute('type')}")

            # Click Next
            print("  ✓ Clicking Next button...")
            next_btn = await page.query_selector('button:has-text("Next"), button[type="submit"]')
            if next_btn:
                await next_btn.click()
                await page.wait_for_timeout(3000)
                print("    ✅ Clicked Next")
            else:
                print("    ⚠️  Next button not found")

            # Wait for password field
            print("  ✓ Waiting for password field...")
            try:
                await page.wait_for_selector('input[type="password"]', timeout=5000)
                password_input = await page.query_selector('input[type="password"]')
                if password_input:
                    await password_input.fill(PASSWORD)
                    await page.wait_for_timeout(500)
                    print(f"    ✅ Entered password")
                else:
                    print("    ⚠️  Password input not found")
            except Exception as e:
                print(f"    ⚠️  Timeout waiting for password: {e}")

            # Click Login/Next
            print("  ✓ Submitting login...")
            login_btn = await page.query_selector('button:has-text("Next"), button:has-text("Login"), button[type="submit"]')
            if login_btn:
                await login_btn.click()
                await page.wait_for_timeout(4000)
                print("    ✅ Submitted login")
            else:
                print("    ⚠️  Login button not found")
                buttons = await page.query_selector_all("button")
                for i, btn in enumerate(buttons):
                    text = await btn.text_content()
                    if text.strip():
                        print(f"      Button {i}: {text.strip()}")

        # Step 3: Verify we're logged in
        print("\nStep 3️⃣ : Verifying login...")
        await page.wait_for_timeout(2000)
        current_url = page.url
        print(f"  Current URL: {current_url}")

        if "insurance" in current_url:
            print("  ✅ Successfully logged in!\n")
        else:
            print("  ⚠️  May not be logged in yet")

        # Step 4: Navigate to Data → Sources
        print("Step 4️⃣ : Navigating to Data → Sources...")

        # Take screenshot first
        await page.screenshot(path="/tmp/1_after_login.png")

        # Look for Data in sidebar
        try:
            # Try different selectors for Data link
            data_link = None
            for selector in ['a:has-text("Data")', 'button:has-text("Data")', '[href*="data"]']:
                data_link = await page.query_selector(selector)
                if data_link:
                    break

            if data_link:
                await data_link.click()
                await page.wait_for_timeout(1500)
                print("  ✅ Clicked Data menu")
            else:
                print("  ⚠️  Data link not found, trying to navigate directly...")
                await page.goto(f"{MIMIR_URL}/insurance/data", wait_until="domcontentloaded")
                await page.wait_for_timeout(1500)
                print("  ✅ Navigated to Data page")

        except Exception as e:
            print(f"  ⚠️  Error navigating Data: {e}")

        await page.screenshot(path="/tmp/2_data_page.png")

        # Step 5: Create Sources
        print("\nStep 5️⃣ : Creating Sources...")

        for i, source in enumerate(SOURCES, 1):
            print(f"\n  Source {i}: {source['name']}")

            try:
                # Look for Add Source button
                add_btn = await page.query_selector('button:has-text("Add Source"), a:has-text("Add Source")')

                if add_btn:
                    await add_btn.click()
                    await page.wait_for_timeout(2000)
                    print(f"    ✅ Clicked Add Source")

                    await page.screenshot(path=f"/tmp/3_source_{i}_form.png")

                    # Fill form fields
                    # Look for name input
                    name_input = await page.query_selector('input[placeholder*="name"], input[placeholder*="Name"]')
                    if name_input:
                        await name_input.fill(source['name'])
                        print(f"    ✅ Filled name: {source['name']}")
                    else:
                        print(f"    ⚠️  Name field not found")

                    await page.wait_for_timeout(500)

                    # Look for collection input
                    coll_input = await page.query_selector('input[placeholder*="collection"], input[placeholder*="Collection"]')
                    if coll_input:
                        await coll_input.fill(source['collection'])
                        print(f"    ✅ Filled collection: {source['collection']}")
                    else:
                        print(f"    ⚠️  Collection field not found")

                    await page.wait_for_timeout(500)

                    # Look for tenant input
                    tenant_input = await page.query_selector('input[placeholder*="tenant"], input[placeholder*="Tenant"]')
                    if tenant_input:
                        await tenant_input.fill("asgard_insurance")
                        print(f"    ✅ Filled tenant: asgard_insurance")
                    else:
                        print(f"    ⚠️  Tenant field not found")

                    await page.wait_for_timeout(500)

                    # Look for Save button
                    save_btn = await page.query_selector('button:has-text("Save"), button:has-text("Create"), button:has-text("OK")')
                    if save_btn:
                        await save_btn.click()
                        await page.wait_for_timeout(2000)
                        print(f"    ✅ Clicked Save")
                    else:
                        print(f"    ⚠️  Save button not found")
                        buttons = await page.query_selector_all("button")
                        print(f"       Available buttons:")
                        for btn in buttons:
                            text = await btn.text_content()
                            if text.strip():
                                print(f"         - {text.strip()}")

                    await page.screenshot(path=f"/tmp/4_source_{i}_created.png")

                else:
                    print(f"    ⚠️  Add Source button not found")

            except Exception as e:
                print(f"    ❌ Error creating source: {e}")

        # Step 6: Final screenshot
        print("\n" + "="*70)
        await page.screenshot(path="/tmp/5_final_overview.png")
        print("✅ Sources creation complete!")
        print("="*70)

        print("\n📸 Screenshots saved:")
        print("  - /tmp/1_after_login.png")
        print("  - /tmp/2_data_page.png")
        print("  - /tmp/3_source_*_form.png")
        print("  - /tmp/4_source_*_created.png")
        print("  - /tmp/5_final_overview.png")

        print("\n💡 Browser will stay open for 30 seconds (verify in UI)")
        await page.wait_for_timeout(30000)

        await browser.close()
        print("\n👋 Done!\n")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n⏹️  Cancelled by user")
    except Exception as e:
        print(f"\n\n❌ Error: {e}")
