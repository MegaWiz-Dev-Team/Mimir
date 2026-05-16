# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: ocr-annotation.spec.ts >> OCR Annotation Feature >> should display datasets or empty state
- Location: e2e/ocr-annotation.spec.ts:35:7

# Error details

```
Error: expect(received).toBeTruthy()

Received: false
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - navigation [ref=e2]:
    - generic [ref=e3]:
      - generic [ref=e4]:
        - link "Asgard Mimir" [ref=e5] [cursor=pointer]:
          - /url: /
        - generic [ref=e6]:
          - link "Overview" [ref=e7] [cursor=pointer]:
            - /url: /
            - img [ref=e8]
            - text: Overview
          - button "Data" [ref=e14]:
            - img [ref=e15]
            - text: Data
            - img [ref=e19]
          - button "AI" [ref=e22]:
            - img [ref=e23]
            - text: AI
            - img [ref=e31]
          - button "Analytics" [ref=e34]:
            - img [ref=e35]
            - text: Analytics
            - img [ref=e37]
      - generic [ref=e39]:
        - generic [ref=e40]:
          - generic [ref=e41]: "Tenant:"
          - combobox [ref=e42]:
            - option "asgard-insurance" [selected]
            - option "Mega Care"
            - option "Default Tenant"
            - option "MegaCare Hospital"
            - option "Insurance Product Platform"
        - generic "Test Annotator" [ref=e43]
        - button "Logout" [ref=e44]:
          - img [ref=e45]
  - main [ref=e48]:
    - generic [ref=e50]:
      - heading "OCR Annotation" [level=1] [ref=e51]
      - generic [ref=e53]: No datasets available. Create a dataset in evaluations first.
  - button "Mimir Assistant & Feedback" [ref=e54]:
    - img [ref=e55]
  - button [ref=e59]:
    - img [ref=e60]
  - button "Open Next.js Dev Tools" [ref=e67] [cursor=pointer]:
    - img [ref=e68]
  - alert [ref=e71]
```

# Test source

```ts
  1   | import { test, expect, Page } from '@playwright/test';
  2   | 
  3   | async function authenticate(page: Page, tenantId: string = 'asgard-insurance') {
  4   |   await page.context().addCookies([
  5   |     { name: 'access_token', value: 'test-jwt-token', url: 'http://localhost:3001' },
  6   |     { name: 'tenant_id', value: tenantId, url: 'http://localhost:3001' },
  7   |     { name: 'user_role', value: 'annotator', url: 'http://localhost:3001' },
  8   |     { name: 'user_name', value: 'Test Annotator', url: 'http://localhost:3001' },
  9   |   ]);
  10  | }
  11  | 
  12  | test.describe('OCR Annotation Feature', () => {
  13  |   test.beforeEach(async ({ page }) => {
  14  |     await authenticate(page);
  15  |   });
  16  | 
  17  |   test('should navigate to annotation page', async ({ page }) => {
  18  |     await page.goto('/');
  19  |     const annotationLink = page.locator('a, button', { hasText: /annotation/i }).first();
  20  |     if (await annotationLink.isVisible()) {
  21  |       await annotationLink.click();
  22  |       await expect(page).toHaveURL(/\/syn-ocr\/annotation/);
  23  |     }
  24  |   });
  25  | 
  26  |   test('should load annotation page', async ({ page }) => {
  27  |     await page.goto('/syn-ocr/annotation');
  28  |     await page.waitForLoadState('networkidle');
  29  | 
  30  |     // Page should load without errors
  31  |     const content = page.locator('main, div[role="main"]');
  32  |     await expect(content).toBeTruthy();
  33  |   });
  34  | 
  35  |   test('should display datasets or empty state', async ({ page }) => {
  36  |     await page.goto('/syn-ocr/annotation');
  37  |     await page.waitForLoadState('networkidle');
  38  | 
  39  |     const datasetList = page.locator('[data-testid="dataset-list"]');
  40  |     const noData = page.locator('text=No datasets');
  41  | 
  42  |     // Either datasets load or no datasets message shows
  43  |     const either = await Promise.race([
  44  |       datasetList.isVisible(),
  45  |       noData.isVisible(),
  46  |     ]).catch(() => false);
  47  | 
> 48  |     expect(either).toBeTruthy();
      |                    ^ Error: expect(received).toBeTruthy()
  49  |   });
  50  | 
  51  |   test('should have clickable annotate buttons if datasets exist', async ({ page }) => {
  52  |     await page.goto('/syn-ocr/annotation');
  53  |     await page.waitForLoadState('networkidle');
  54  | 
  55  |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  56  |     const count = await annotateBtns.count();
  57  | 
  58  |     if (count > 0) {
  59  |       const firstBtn = annotateBtns.first();
  60  |       await expect(firstBtn).toBeEnabled();
  61  |     }
  62  |   });
  63  | 
  64  |   test('should display annotation form when task loads', async ({ page }) => {
  65  |     await page.goto('/syn-ocr/annotation');
  66  |     await page.waitForLoadState('networkidle');
  67  | 
  68  |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  69  |     if (await annotateBtns.first().isEnabled()) {
  70  |       await annotateBtns.first().click();
  71  |       await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
  72  | 
  73  |       // Verify form fields exist
  74  |       await expect(page.locator('[data-testid="ground-truth-input"]')).toBeTruthy();
  75  |       await expect(page.locator('[data-testid="confidence-select"]')).toBeTruthy();
  76  |     }
  77  |   });
  78  | 
  79  |   test('should allow filling and saving annotation', async ({ page }) => {
  80  |     await page.goto('/syn-ocr/annotation');
  81  |     await page.waitForLoadState('networkidle');
  82  | 
  83  |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  84  |     if (await annotateBtns.first().isEnabled()) {
  85  |       await annotateBtns.first().click();
  86  |       await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
  87  | 
  88  |       // Fill form
  89  |       const groundTruth = page.locator('[data-testid="ground-truth-input"]');
  90  |       await groundTruth.fill('Test OCR Text');
  91  | 
  92  |       const confidence = page.locator('[data-testid="confidence-select"]');
  93  |       await confidence.selectOption('high');
  94  | 
  95  |       // Verify form can be interacted with
  96  |       const value = await groundTruth.inputValue();
  97  |       expect(value).toBe('Test OCR Text');
  98  |     }
  99  |   });
  100 | 
  101 |   test('should display annotator information', async ({ page }) => {
  102 |     await page.goto('/syn-ocr/annotation');
  103 |     await page.waitForLoadState('networkidle');
  104 | 
  105 |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  106 |     if (await annotateBtns.first().isEnabled()) {
  107 |       await annotateBtns.first().click();
  108 |       await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
  109 | 
  110 |       const annotatorInfo = page.locator('[data-testid="annotator-info"]');
  111 |       if (await annotatorInfo.isVisible()) {
  112 |         await expect(annotatorInfo).toContainText('you');
  113 |       }
  114 |     }
  115 |   });
  116 | 
  117 |   test('should have action buttons', async ({ page }) => {
  118 |     await page.goto('/syn-ocr/annotation');
  119 |     await page.waitForLoadState('networkidle');
  120 | 
  121 |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  122 |     if (await annotateBtns.first().isEnabled()) {
  123 |       await annotateBtns.first().click();
  124 |       await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
  125 | 
  126 |       // Check buttons exist
  127 |       const skipBtn = page.locator('[data-testid="skip-btn"]');
  128 |       const saveDraftBtn = page.locator('[data-testid="save-draft-btn"]');
  129 |       const completeBtn = page.locator('[data-testid="complete-btn"]');
  130 | 
  131 |       const skipExists = await skipBtn.isVisible().catch(() => false);
  132 |       const saveDraftExists = await saveDraftBtn.isVisible().catch(() => false);
  133 |       const completeExists = await completeBtn.isVisible().catch(() => false);
  134 | 
  135 |       expect(skipExists || saveDraftExists || completeExists).toBeTruthy();
  136 |     }
  137 |   });
  138 | 
  139 |   test('should show confidence options', async ({ page }) => {
  140 |     await page.goto('/syn-ocr/annotation');
  141 |     await page.waitForLoadState('networkidle');
  142 | 
  143 |     const annotateBtns = page.locator('[data-testid="annotate-btn"]');
  144 |     if (await annotateBtns.first().isEnabled()) {
  145 |       await annotateBtns.first().click();
  146 |       await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
  147 | 
  148 |       const confidenceSelect = page.locator('[data-testid="confidence-select"]');
```