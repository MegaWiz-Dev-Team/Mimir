# SI-04-2: Sprint 2 Test Script (Data Isolation, Vector Management, Settings)

## Overview
This document outlines the test cases for Sprint 2 of Project Mimir. The focus is on Data Isolation (Multi-Tenancy), Vector Management (managing Qdrant vectors via UI), and Tenant Settings (managing the tenant's name).

## Test Environment
- Branch: `HEAD` or staging
- Service: `ro-ai-bridge` (Backend), `ro-ai-dashboard` (Frontend)
- DB: MariaDB, Qdrant
- Credentials: Admin User with a specific Tenant ID

## Test Cases

### TC_SP2_01: Tenant Settings Page Loading
**Description:** Verify that the Settings page loads properly and fetches the current tenant's data.
**Pre-condition:** Logged in as an Admin user.
**Steps:**
1. Navigate to `/settings` in the ro-ai-dashboard.
2. Verify that the current Tenant's Name is displayed in the input field.
3. Verify that the actual Tenant ID is displayed in the disabled field.
**Expected Result:** Data loads correctly without errors.

### TC_SP2_02: Update Tenant Name
**Description:** Verify that an Admin can update their Tenant's name successfully.
**Pre-condition:** Logged in as an Admin user and on the `/settings` page.
**Steps:**
1. Change the text in the "Tenant Name" input field.
2. Click the "Save Changes" button.
3. Observe the success alert.
4. Refresh the page.
**Expected Result:** The new name persists and is fetched properly after saving.

### TC_SP2_03: Data Isolation - API Filtering
**Description:** Verify that APIs respect the JWT access token's `tenant_id` claim when retrieving or searching records.
**Pre-condition:** Logged in as a user assigned to Tenant A.
**Steps:**
1. Use an API client or the UI to fetch records from a tenant-aware endpoint (e.g., Vector Search or QA/QC results).
2. Look at the filtered results.
**Expected Result:** Only data associated with Tenant A should be returned.
**Status:** [Pass] - Verified via automated backend integration tests. API strictly enforces TenantContext logic.

### TC_SP2_04: Vector Management UI Updates
**Description:** Verify that vector management UI correctly allows removing entries and viewing score badges.
**Pre-condition:** Logged in as an Admin user, navigated to the Vector Management page.
**Steps:**
1. Perform a basic vector search.
2. Verify that the expandable row allows inspecting raw document metadata.
3. Verify the Similarity Score badges display corresponding colors based on threshold.
4. Click the "Delete Vector" 🗑️ button on a specific entry and confirm.
**Expected Result:** Entry is deleted from Qdrant, and the results list updates accurately.
**Status:** [Pass] - Automated endpoints tested and UI validated to contain expanders, badges, and delete functionality.
