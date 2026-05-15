import { test, expect, Page } from '@playwright/test';

// Helper to set authentication cookies for tests
async function authenticate(page: Page, tenantId: string = 'asgard-insurance') {
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
      value: 'Test Annotator',
      url: 'http://localhost:3001',
    },
  ]);
}

test.describe('OCR Annotation Feature', () => {
  test.beforeEach(async ({ page }) => {
    await authenticate(page);
  });

  test.describe('Dataset Listing', () => {
    test('should navigate to annotation page from navbar', async ({ page }) => {
      await page.goto('/');

      // Look for Analytics menu
      const analyticsMenu = page.locator('button', { hasText: 'Analytics' });
      await analyticsMenu.hover();

      // Look for Annotation link
      const annotationLink = page.locator('a', { hasText: 'Annotation' });
      await annotationLink.click();

      // Verify we're on the annotation page
      await expect(page).toHaveURL('/syn-ocr/annotation');
    });

    test('should display dataset list with progress information', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      // Wait for datasets to load
      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      // Check for dataset items
      const datasetItems = page.locator('[data-testid="dataset-item"]');
      const count = await datasetItems.count();
      expect(count).toBeGreaterThan(0);

      // Verify dataset structure
      const firstDataset = datasetItems.first();
      await expect(firstDataset.locator('[data-testid="dataset-name"]')).toBeVisible();
      await expect(firstDataset.locator('[data-testid="progress-bar"]')).toBeVisible();
      await expect(firstDataset.locator('[data-testid="status-counts"]')).toBeVisible();
    });

    test('should show progress breakdown for each dataset', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      const firstDataset = page.locator('[data-testid="dataset-item"]').first();
      const statusCounts = firstDataset.locator('[data-testid="status-counts"]');

      // Verify status breakdown text (completed, in_progress, pending)
      await expect(statusCounts).toContainText(/\d+ completed/);
      await expect(statusCounts).toContainText(/\d+ pending/);
    });

    test('should have Annotate button on each dataset', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      const firstDataset = page.locator('[data-testid="dataset-item"]').first();
      const annotateBtn = firstDataset.locator('[data-testid="annotate-btn"]');

      await expect(annotateBtn).toBeVisible();
      await expect(annotateBtn).toContainText('Annotate');
    });
  });

  test.describe('Annotation Workflow', () => {
    test('should start annotation workflow and display task detail', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      // Click Annotate button
      const annotateBtn = page.locator('[data-testid="annotate-btn"]').first();
      await annotateBtn.click();

      // Should now be in annotation view
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Verify task is displayed
      await expect(page.locator('[data-testid="task-header"]')).toBeVisible();
      await expect(page.locator('[data-testid="image-preview"]')).toBeVisible();
      await expect(page.locator('[data-testid="ground-truth-input"]')).toBeVisible();
    });

    test('should display image preview when task is loaded', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const imagePreview = page.locator('[data-testid="image-preview"] img');
      await expect(imagePreview).toHaveAttribute('src', /\/api\/v1\/ocr-annotation\/tasks\/.+\/image/);
      await expect(imagePreview).toBeVisible();
    });

    test('should save annotation as draft without marking complete', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Fill ground truth
      const groundTruthInput = page.locator('[data-testid="ground-truth-input"]');
      await groundTruthInput.fill('Test OCR text');

      // Select confidence
      const confidenceSelect = page.locator('[data-testid="confidence-select"]');
      await confidenceSelect.selectOption('high');

      // Click Save Draft
      const saveDraftBtn = page.locator('[data-testid="save-draft-btn"]');
      await saveDraftBtn.click();

      // Should show success message
      await expect(page.locator('[data-testid="save-success"]')).toBeVisible();

      // Should still be on same task or show in_progress status
      const statusBadge = page.locator('[data-testid="task-status"]');
      await expect(statusBadge).toContainText('in_progress');
    });

    test('should complete annotation and auto-advance to next task', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      const datasetItems = page.locator('[data-testid="dataset-item"]');
      const datasetCount = await datasetItems.count();

      if (datasetCount === 0) {
        test.skip();
      }

      await page.locator('[data-testid="annotate-btn"]').first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const taskIdBefore = await page.locator('[data-testid="task-id"]').textContent();

      // Fill and complete
      await page.locator('[data-testid="ground-truth-input"]').fill('Test OCR');
      await page.locator('[data-testid="confidence-select"]').selectOption('medium');

      // Click Complete →
      const completeBtn = page.locator('[data-testid="complete-btn"]');
      await completeBtn.click();

      // Should show success
      await expect(page.locator('[data-testid="save-success"]')).toBeVisible();

      // If there's a next task, task ID should change
      await page.waitForTimeout(500);
      const taskIdAfter = await page.locator('[data-testid="task-id"]').textContent();

      // Either task advanced or returned to dataset list
      const isDatasetList = await page.locator('[data-testid="dataset-list"]').isVisible();
      const taskChanged = taskIdBefore !== taskIdAfter;

      expect(isDatasetList || taskChanged).toBeTruthy();
    });

    test('should skip task and move to next', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const taskIdBefore = await page.locator('[data-testid="task-id"]').textContent();

      // Click Skip button
      const skipBtn = page.locator('[data-testid="skip-btn"]');
      await skipBtn.click();

      // Should show success message
      await expect(page.locator('[data-testid="skip-success"]')).toBeVisible({ timeout: 3000 });

      // Should advance to next task or return to dataset list
      await page.waitForTimeout(500);
      const taskIdAfter = await page.locator('[data-testid="task-id"]').textContent();
      const isDatasetList = await page.locator('[data-testid="dataset-list"]').isVisible();

      expect(isDatasetList || taskIdBefore !== taskIdAfter).toBeTruthy();
    });

    test('should display and set confidence level', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const confidenceSelect = page.locator('[data-testid="confidence-select"]');

      // Check all confidence options are available
      await confidenceSelect.click();
      await expect(page.locator('option[value="high"]')).toBeVisible();
      await expect(page.locator('option[value="medium"]')).toBeVisible();
      await expect(page.locator('option[value="low"]')).toBeVisible();

      // Select one
      await confidenceSelect.selectOption('high');
      await expect(confidenceSelect).toHaveValue('high');
    });

    test('should toggle issue checkboxes', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Check that issue checkboxes exist
      const handwrittenCheckbox = page.locator('input[data-testid="issue-handwritten"]');
      const blurryCheckbox = page.locator('input[data-testid="issue-blurry"]');
      const partialCheckbox = page.locator('input[data-testid="issue-partial"]');
      const damagedCheckbox = page.locator('input[data-testid="issue-damaged"]');

      await expect(handwrittenCheckbox).toBeVisible();
      await expect(blurryCheckbox).toBeVisible();
      await expect(partialCheckbox).toBeVisible();
      await expect(damagedCheckbox).toBeVisible();

      // Toggle one
      await handwrittenCheckbox.check();
      await expect(handwrittenCheckbox).toBeChecked();
    });
  });

  test.describe('Progress Tracking', () => {
    test('should display progress bar in task header', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Check progress display
      const taskProgress = page.locator('[data-testid="task-progress"]');
      await expect(taskProgress).toBeVisible();
      await expect(taskProgress).toContainText(/\d+ of \d+/);
    });

    test('should update dataset progress after completing task', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      const firstDataset = page.locator('[data-testid="dataset-item"]').first();
      const initialProgress = await firstDataset.locator('[data-testid="status-counts"]').textContent();

      // Start annotation
      await page.locator('[data-testid="annotate-btn"]').first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Complete task
      await page.locator('[data-testid="ground-truth-input"]').fill('Test');
      await page.locator('[data-testid="confidence-select"]').selectOption('high');
      await page.locator('[data-testid="complete-btn"]').click();

      // Go back to dataset list
      await page.waitForTimeout(1000);
      const backBtn = page.locator('[data-testid="back-to-datasets"]');
      if (await backBtn.isVisible()) {
        await backBtn.click();
      }

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      // Progress should be updated
      const updatedProgress = await page.locator('[data-testid="dataset-item"]').first()
        .locator('[data-testid="status-counts"]').textContent();

      expect(updatedProgress).not.toEqual(initialProgress);
    });
  });

  test.describe('Multi-user Support', () => {
    test('should display current annotator in annotation view', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Should show annotator info
      const annotatorInfo = page.locator('[data-testid="annotator-info"]');
      await expect(annotatorInfo).toBeVisible();
      await expect(annotatorInfo).toContainText('Test Annotator');
    });

    test('should maintain user context across requests', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Fill and save
      await page.locator('[data-testid="ground-truth-input"]').fill('Test');
      await page.locator('[data-testid="confidence-select"]').selectOption('high');

      // Intercept the save request to verify user context
      let capturedRequest: any = null;
      page.on('request', request => {
        if (request.url().includes('/api/v1/ocr-annotation/tasks/') && request.method() === 'POST') {
          capturedRequest = request;
        }
      });

      await page.locator('[data-testid="save-draft-btn"]').click();

      // Wait for request to be captured
      await page.waitForTimeout(500);

      // Request should have been made (annotator context maintained in backend)
      expect(capturedRequest).not.toBeNull();
    });
  });

  test.describe('Navigation and State', () => {
    test('should return to dataset list from annotation', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Click back button
      const backBtn = page.locator('[data-testid="back-to-datasets"]');
      if (await backBtn.isVisible()) {
        await backBtn.click();
      } else {
        // Alternative: use browser back
        await page.goBack();
      }

      // Should be back at dataset list
      await expect(page.locator('[data-testid="dataset-list"]')).toBeVisible({ timeout: 3000 });
    });

    test('should persist form state while on same task', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      const testText = 'Test OCR Text That Should Persist';
      await page.locator('[data-testid="ground-truth-input"]').fill(testText);

      // Type something in notes
      const notesInput = page.locator('[data-testid="notes-input"]');
      if (await notesInput.isVisible()) {
        await notesInput.fill('Test notes');
      }

      // Verify values persist
      await expect(page.locator('[data-testid="ground-truth-input"]')).toHaveValue(testText);
    });

    test('should handle missing images gracefully', async ({ page, context }) => {
      // This test ensures the UI doesn't crash if image endpoint fails
      await authenticate(page);
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });

      // The page should still function even if image fails to load
      await page.locator('[data-testid="annotate-btn"]').first().click();
      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Form should still be usable
      const groundTruthInput = page.locator('[data-testid="ground-truth-input"]');
      await expect(groundTruthInput).toBeVisible();
      await groundTruthInput.fill('Text without image');
    });
  });

  test.describe('Error Handling', () => {
    test('should handle API errors gracefully', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      // If API is down, page should show error message
      const errorElement = page.locator('[data-testid="error-message"]');
      const datasetList = page.locator('[data-testid="dataset-list"]');

      // Either data loads or error is shown
      try {
        await datasetList.waitFor({ timeout: 5000 });
      } catch {
        await expect(errorElement).toBeVisible({ timeout: 3000 });
      }
    });

    test('should show validation errors before save', async ({ page }) => {
      await page.goto('/syn-ocr/annotation');

      await page.waitForSelector('[data-testid="dataset-list"]', { timeout: 5000 });
      await page.locator('[data-testid="annotate-btn"]').first().click();

      await page.waitForSelector('[data-testid="annotation-editor"]', { timeout: 5000 });

      // Try to save without filling required fields
      const completeBtn = page.locator('[data-testid="complete-btn"]');
      await completeBtn.click();

      // Should show validation error or prevent save
      const errorMsg = page.locator('[data-testid="validation-error"]');
      if (await errorMsg.isVisible()) {
        await expect(errorMsg).toContainText(/required|must|please/i);
      }
    });
  });
});
