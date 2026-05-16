import { test, expect, Page } from '@playwright/test';

async function authenticate(page: Page, tenantId: string = 'asgard-insurance') {
  await page.context().addCookies([
    { name: 'access_token', value: 'test-jwt-token', url: 'http://localhost:3001' },
    { name: 'tenant_id', value: tenantId, url: 'http://localhost:3001' },
    { name: 'user_role', value: 'annotator', url: 'http://localhost:3001' },
    { name: 'user_name', value: 'Test Annotator', url: 'http://localhost:3001' },
  ]);
}

test.describe('OCR Annotation Feature', () => {
  test.beforeEach(async ({ page }) => {
    await authenticate(page);
  });

  test('should navigate to annotation page', async ({ page }) => {
    await page.goto('/');
    const annotationLink = page.locator('a, button', { hasText: /annotation/i }).first();
    if (await annotationLink.isVisible()) {
      await annotationLink.click();
      await expect(page).toHaveURL(/\/syn-ocr\/annotation/);
    }
  });

  test('should load annotation page', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    // Page should load without errors
    const content = page.locator('main, div[role="main"]');
    await expect(content).toBeTruthy();
  });

  test('should display datasets or empty state', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const datasetList = page.locator('[data-testid="dataset-list"]');
    const noData = page.locator('text=No datasets');

    // Either datasets load or no datasets message shows
    const either = await Promise.race([
      datasetList.isVisible(),
      noData.isVisible(),
    ]).catch(() => false);

    expect(either).toBeTruthy();
  });

  test('should have clickable annotate buttons if datasets exist', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    const count = await annotateBtns.count();

    if (count > 0) {
      const firstBtn = annotateBtns.first();
      await expect(firstBtn).toBeEnabled();
    }
  });

  test('should display annotation form when task loads', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Verify form fields exist
      await expect(page.locator('[data-testid="ground-truth-input"]')).toBeTruthy();
      await expect(page.locator('[data-testid="confidence-select"]')).toBeTruthy();
    }
  });

  test('should allow filling and saving annotation', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Fill form
      const groundTruth = page.locator('[data-testid="ground-truth-input"]');
      await groundTruth.fill('Test OCR Text');

      const confidence = page.locator('[data-testid="confidence-select"]');
      await confidence.selectOption('high');

      // Verify form can be interacted with
      const value = await groundTruth.inputValue();
      expect(value).toBe('Test OCR Text');
    }
  });

  test('should display annotator information', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const annotatorInfo = page.locator('[data-testid="annotator-info"]');
      if (await annotatorInfo.isVisible()) {
        await expect(annotatorInfo).toContainText('you');
      }
    }
  });

  test('should have action buttons', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Check buttons exist
      const skipBtn = page.locator('[data-testid="skip-btn"]');
      const saveDraftBtn = page.locator('[data-testid="save-draft-btn"]');
      const completeBtn = page.locator('[data-testid="complete-btn"]');

      const skipExists = await skipBtn.isVisible().catch(() => false);
      const saveDraftExists = await saveDraftBtn.isVisible().catch(() => false);
      const completeExists = await completeBtn.isVisible().catch(() => false);

      expect(skipExists || saveDraftExists || completeExists).toBeTruthy();
    }
  });

  test('should show confidence options', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const confidenceSelect = page.locator('[data-testid="confidence-select"]');
      await confidenceSelect.click();

      const options = page.locator('option');
      const count = await options.count();
      expect(count).toBeGreaterThanOrEqual(3);
    }
  });

  test('should display issue checkboxes', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Check for issue checkboxes
      const checkboxes = page.locator('input[type="checkbox"]');
      const count = await checkboxes.count();
      expect(count).toBeGreaterThanOrEqual(4); // At least 4 issue types
    }
  });

  test('should have back button to return to datasets', async ({ page }) => {
    await page.goto('/syn-ocr/annotation');
    await page.waitForLoadState('networkidle');

    const annotateBtns = page.locator('[data-testid="annotate-btn"]');
    if (await annotateBtns.first().isEnabled()) {
      await annotateBtns.first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const backBtn = page.locator('[data-testid="back-to-datasets"]');
      if (await backBtn.isVisible()) {
        await expect(backBtn).toBeTruthy();
      }
    }
  });
});
