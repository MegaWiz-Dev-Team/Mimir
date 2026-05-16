#!/usr/bin/env python3
"""Playwright script to create insurance sources in Mimir UI (with SSO auth)."""

import asyncio
import os
from playwright.async_api import async_playwright

MIMIR_URL = "https://mimir.asgard.internal"

async def create_sources():
    """Create sources for S2 insurance products."""

    # Get credentials from environment or use defaults
    username = os.getenv("ASGARD_USER", "admin")
    password = os.getenv("ASGARD_PASSWORD", "")

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=False, args=[
            "--no-sandbox",
            "--disable-gpu",
            "--ignore-certificate-errors",
        ])
        context = await browser.new_context(
            ignore_https_errors=True
        )
        page = await context.new_page()

        print(f"📱 Opening Mimir at {MIMIR_URL}")
        await page.goto(MIMIR_URL, wait_until="domcontentloaded")
        await page.wait_for_timeout(2000)

        # Take screenshot before auth
        await page.screenshot(path="/tmp/1_before_auth.png")
        print("📸 Screenshot 1: Before auth")

        # Check if we're on login page
        current_url = page.url
        print(f"📍 Current URL: {current_url}")

        # If on Zitadel login, handle SSO
        if "sso.asgard.internal" in current_url or "login" in current_url:
            print("\n🔐 SSO Login Required")
            print("Looking for username field...")

            try:
                # Find username input
                username_input = await page.query_selector('input[placeholder*="username"]')
                if not username_input:
                    username_input = await page.query_selector('input[name*="username"]')
                if not username_input:
                    username_input = await page.query_selector('input[type="text"]')

                if username_input:
                    print(f"✅ Found username input, entering: {username}")
                    await username_input.fill(username)
                    await page.wait_for_timeout(500)

                    # Click Next button
                    next_btn = await page.query_selector('button:has-text("Next")')
                    if next_btn:
                        await next_btn.click()
                        await page.wait_for_timeout(2000)
                        print("✅ Clicked Next")
                    else:
                        print("⚠️  No Next button found")

                    await page.screenshot(path="/tmp/2_after_username.png")
                    print("📸 Screenshot 2: After username")

                else:
                    print("⚠️  Username input not found")
                    # Print all inputs
                    inputs = await page.query_selector_all("input")
                    for i, inp in enumerate(inputs):
                        placeholder = await inp.get_attribute("placeholder")
                        inp_type = await inp.get_attribute("type")
                        print(f"  Input {i}: type={inp_type}, placeholder={placeholder}")

            except Exception as e:
                print(f"❌ Error during login: {e}")

            # Try password field
            try:
                await page.wait_for_timeout(1000)
                password_input = await page.query_selector('input[type="password"]')
                if password_input and password:
                    print(f"✅ Found password input, entering password")
                    await password_input.fill(password)
                    await page.wait_for_timeout(500)

                    # Click Next or Login
                    login_btn = await page.query_selector('button:has-text("Next")') or \
                               await page.query_selector('button:has-text("Login")')
                    if login_btn:
                        await login_btn.click()
                        await page.wait_for_timeout(3000)
                        print("✅ Clicked Login/Next")
                    else:
                        print("⚠️  No Login button found")

                    await page.screenshot(path="/tmp/3_after_password.png")
                    print("📸 Screenshot 3: After password")

                elif not password_input:
                    print("⚠️  Password input not found yet")

            except Exception as e:
                print(f"⚠️  Error with password: {e}")

            # Wait for redirect to Mimir
            try:
                await page.wait_for_load_state("networkidle", timeout=5000)
            except:
                pass
            await page.wait_for_timeout(2000)

        # Now we should be in Mimir
        current_url = page.url
        print(f"\n✅ Current URL: {current_url}")

        # Take screenshot after auth
        await page.screenshot(path="/tmp/4_after_auth.png")
        print("📸 Screenshot 4: After authentication")

        # Look for Data menu
        try:
            print("\n🔍 Looking for Data menu...")
            await page.wait_for_selector("text=Data", timeout=5000)
            await page.click("text=Data")
            await page.wait_for_timeout(1000)
            print("✅ Clicked Data menu")

            # Look for Sources
            await page.wait_for_selector("text=Sources", timeout=5000)
            await page.click("text=Sources")
            await page.wait_for_timeout(1000)
            print("✅ Clicked Sources")

            await page.screenshot(path="/tmp/5_sources_page.png")
            print("📸 Screenshot 5: Sources page")

            # Look for Add Source button
            add_btn = await page.query_selector("text=Add Source")
            if add_btn:
                print("✅ Found 'Add Source' button")
                await add_btn.click()
                await page.wait_for_timeout(2000)
                print("✅ Clicked Add Source")

                await page.screenshot(path="/tmp/6_add_source_form.png")
                print("📸 Screenshot 6: Add Source form")

        except Exception as e:
            print(f"⚠️  Error navigating: {e}")

        # Print page info
        content = await page.content()
        print(f"\n📄 Page HTML length: {len(content)} chars")

        await page.screenshot(path="/tmp/7_final_state.png")
        print("📸 Screenshot 7: Final state")

        print("\n" + "="*60)
        print("Screenshots saved to /tmp/")
        print("="*60)

        await browser.close()

if __name__ == "__main__":
    asyncio.run(create_sources())
