---
title: Sprint 32 Test Script
status: Completed
date: 2026-03-30
author: System
sprint: 32
---

# Test Script (SI) - Sprint 32

## 1. Introduction
This document details the Test-Driven Development (TDD) artifacts for the Dynamic LLM Routing UI update within the Mimir Dashboard application. 

## 2. Test Cases

### 2.1 AI Models Tab
**File:** `src/app/settings/components/AIModelsTab.test.tsx`  
**Description:** Validates that the default config fields are successfully rendering from the backend config object.  
**Steps:**
1. Render `<AIModelsTab>` wrapped with the mocked `SettingsTabProps`.
2. Check if `Default Provider & Model` text is in the document.
3. Assert that the `pipeline_evaluator` label exists.
4. Fire `change` event on the Default Provider dropdown.
5. Expect `mockSetConfig` to have been called with the updated hierarchy.

### 2.2 Search Tab
**File:** `src/app/settings/components/SearchTab.test.tsx`  
**Description:** Validates that the unified structure for Vector Encodings correctly displays the Heimdall BGE representations.  
**Steps:**
1. Render `<SearchTab>` pointing to `config.llm_config.embedding`.
2. Assert value selection renders "BGE-M3 (MLX)".
3. Change provider dropdown to "openai".
4. Expect `mockSetConfig` to trigger.

### 2.3 Security Tab
**File:** `src/app/settings/components/SecurityTab.test.tsx`  
**Description:** Ensure API key management is successfully mapped into `TenantConfig`.  
**Steps:**
1. Render `<SecurityTab>`.
2. Expect `External Provider Credentials` to appear.
3. Find input with value mapped from `heimdall_url`.
4. Fire `change` event simulating a URL update.
5. Expect `mockSetConfig` to be invoked.

All TDD tests have been executed locally satisfying standard front-end pipeline requirements.
