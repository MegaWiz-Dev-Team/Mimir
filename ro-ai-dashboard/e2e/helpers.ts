import { Page, BrowserContext } from '@playwright/test';

/**
 * Helper functions for OCR Annotation E2E tests
 */

/**
 * Authenticate a page with mock cookies for testing
 */
export async function authenticate(
  page: Page,
  tenantId: string = 'asgard-insurance',
  userId: string = 'test-user-123',
  userName: string = 'Test Annotator'
) {
  await page.context().addCookies([
    {
      name: 'access_token',
      value: 'test-jwt-token',
      url: 'http://localhost:3001',
    },
    {
      name: 'tenant_id',
      value: tenantId,
      url: 'http://localhost:3001',
    },
    {
      name: 'user_role',
      value: 'annotator',
      url: 'http://localhost:3001',
    },
    {
      name: 'user_name',
      value: userName,
      url: 'http://localhost:3001',
    },
  ]);
}

/**
 * Wait for the dataset list to load and be visible
 */
export async function waitForDatasetList(page: Page, timeout: number = 5000) {
  await page.waitForSelector('[data-testid="dataset-list"]', { timeout });
}

/**
 * Navigate to a dataset and start annotating
 */
export async function startAnnotatingFirstDataset(page: Page) {
  await page.goto('/syn-ocr/annotation');
  await waitForDatasetList(page);
  await page.locator('[data-testid="annotate-btn"]').first().click();
  await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });
}

/**
 * Fill in annotation form and save
 */
export async function fillAndSaveAnnotation(
  page: Page,
  groundTruth: string,
  confidence: 'high' | 'medium' | 'low' = 'high',
  issues: string[] = [],
  notes: string = ''
) {
  // Fill ground truth
  await page.locator('[data-testid="ground-truth-input"]').fill(groundTruth);

  // Set confidence
  await page.locator('[data-testid="confidence-select"]').selectOption(confidence);

  // Check issues
  for (const issue of issues) {
    const testIdMap: Record<string, string> = {
      'Handwritten': 'issue-handwritten',
      'Blurry': 'issue-blurry',
      'Partial': 'issue-partial',
      'Damaged': 'issue-damaged',
    };
    if (testIdMap[issue]) {
      await page.locator(`[data-testid="${testIdMap[issue]}"]`).check();
    }
  }

  // Add notes if provided
  if (notes) {
    await page.locator('[data-testid="notes-input"]').fill(notes);
  }
}

/**
 * Complete current annotation task
 */
export async function completeAnnotation(page: Page) {
  await page.locator('[data-testid="complete-btn"]').click();
  // Wait for save success message
  await page.waitForSelector('[data-testid="save-success"]', { timeout: 3000 });
}

/**
 * Save annotation as draft without completing
 */
export async function saveDraft(page: Page) {
  await page.locator('[data-testid="save-draft-btn"]').click();
  await page.waitForSelector('[data-testid="save-success"]', { timeout: 3000 });
}

/**
 * Skip current annotation task
 */
export async function skipTask(page: Page) {
  await page.locator('[data-testid="skip-btn"]').click();
  await page.waitForSelector('[data-testid="skip-success"]', { timeout: 3000 });
}

/**
 * Get current task ID from the page
 */
export async function getCurrentTaskId(page: Page): Promise<string> {
  return page.locator('[data-testid="task-id"]').textContent() as Promise<string>;
}

/**
 * Get dataset progress info
 */
export async function getDatasetProgress(page: Page) {
  const items = page.locator('[data-testid="dataset-item"]');
  const count = await items.count();

  const datasets = [];
  for (let i = 0; i < count; i++) {
    const item = items.nth(i);
    const name = await item.locator('[data-testid="dataset-name"]').textContent();
    const counts = await item.locator('[data-testid="status-counts"]').textContent();
    datasets.push({ name, counts });
  }

  return datasets;
}

/**
 * Check if annotation editor is visible
 */
export async function isAnnotationEditorVisible(page: Page): Promise<boolean> {
  return page.locator('[data-testid="annotation-editor"]').isVisible();
}

/**
 * Check if dataset list is visible
 */
export async function isDatasetListVisible(page: Page): Promise<boolean> {
  return page.locator('[data-testid="dataset-list"]').isVisible();
}

/**
 * Get image source URL
 */
export async function getImageUrl(page: Page): Promise<string | null> {
  const img = page.locator('[data-testid="image-preview"] img');
  if (await img.isVisible()) {
    return img.getAttribute('src');
  }
  return null;
}

/**
 * Get current ground truth value
 */
export async function getGroundTruthValue(page: Page): Promise<string> {
  return page.locator('[data-testid="ground-truth-input"]').inputValue();
}

/**
 * Get current confidence value
 */
export async function getConfidenceValue(page: Page): Promise<string> {
  return page.locator('[data-testid="confidence-select"]').inputValue();
}

/**
 * Check if a specific issue is checked
 */
export async function isIssueChecked(page: Page, issue: string): Promise<boolean> {
  const testIdMap: Record<string, string> = {
    'Handwritten': 'issue-handwritten',
    'Blurry': 'issue-blurry',
    'Partial': 'issue-partial',
    'Damaged': 'issue-damaged',
  };

  if (!testIdMap[issue]) {
    return false;
  }

  return page.locator(`[data-testid="${testIdMap[issue]}"]`).isChecked();
}

/**
 * Go back to dataset list
 */
export async function backToDatasets(page: Page) {
  await page.locator('[data-testid="back-to-datasets"]').click();
  await waitForDatasetList(page);
}

/**
 * Wait for page to be in specific view
 */
export async function waitForView(page: Page, view: 'datasets' | 'annotate', timeout: number = 5000) {
  if (view === 'datasets') {
    await page.waitForSelector('[data-testid="dataset-list"]', { timeout });
  } else {
    await page.waitForSelector('[data-testid="annotation-editor"]', { timeout });
  }
}
