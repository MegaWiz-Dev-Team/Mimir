# OCR Annotation E2E Tests

Comprehensive end-to-end test suite for the OCR Annotation UI feature using Playwright.

## Setup

Install dependencies:
```bash
npm install
```

## Running Tests

**Run all E2E tests:**
```bash
npm run test:e2e
```

**Run with UI mode (interactive):**
```bash
npm run test:e2e:ui
```

**Run in debug mode:**
```bash
npm run test:e2e:debug
```

**Run a specific test file:**
```bash
npx playwright test e2e/ocr-annotation.spec.ts
```

**Run a specific test group:**
```bash
npx playwright test -g "Dataset Listing"
```

## Test Coverage

The test suite covers the following areas:

### Dataset Listing
- Navigation to annotation page from navbar
- Dataset list display with progress information
- Progress breakdown (completed, in_progress, pending)
- Annotate button availability and functionality

### Annotation Workflow
- Starting annotation from dataset list
- Image preview rendering and loading
- Saving annotations as draft (status=in_progress)
- Completing annotations (status=completed, auto-advance)
- Skipping tasks (status=skipped)
- Confidence level selection (high, medium, low)
- Issue checkbox toggling (Handwritten, Blurry, Partial, Damaged)
- Notes input

### Progress Tracking
- Progress bar display in task header
- Dataset progress updates after task completion
- Task counter (X of Y) accuracy

### Multi-user Support
- Annotator identification and display
- User context maintained across API requests
- Annotator ID persistence in database

### Navigation and State
- Back button returning to dataset list
- Form state persistence while on same task
- Graceful handling of missing images
- Error handling for API failures
- Validation error display

## Test Data Requirements

Tests expect:
- At least one OCR evaluation dataset in the database
- Dataset must have at least one OCR annotation task
- Images must be accessible via the annotation image endpoint

## Authentication

Tests use mock authentication cookies:
- `access_token`: JWT token (mocked)
- `tenant_id`: asgard-insurance (default)
- `user_role`: annotator
- `user_name`: Test Annotator

Modify `authenticate()` helper in the test file to use different credentials if needed.

## Test IDs

The annotation page uses `data-testid` attributes for element selection:

**Dataset List:**
- `dataset-list` - Container for all datasets
- `dataset-item` - Individual dataset card
- `dataset-name` - Dataset name heading
- `annotate-btn` - Annotate button
- `progress-bar` - Progress bar element
- `status-counts` - Status breakdown text

**Annotation Editor:**
- `annotation-editor` - Main annotation form container
- `task-header` - Task header with back button
- `task-id` - Task ID/case label
- `task-progress` - Task progress counter
- `image-preview` - Image container
- `ground-truth-input` - Ground truth textarea
- `confidence-select` - Confidence dropdown
- `issue-handwritten` - Handwritten checkbox
- `issue-blurry` - Blurry checkbox
- `issue-partial` - Partial checkbox
- `issue-damaged` - Damaged checkbox
- `notes-input` - Notes input field
- `annotator-info` - Annotator display
- `task-status` - Current task status badge

**Buttons:**
- `back-to-datasets` - Back button
- `save-draft-btn` - Save Draft button
- `skip-btn` - Skip button
- `complete-btn` - Complete → button

**Feedback:**
- `save-success` - Success message after save
- `skip-success` - Success message after skip
- `validation-error` - Validation error message
- `error-message` - General error message

## Configuration

Playwright configuration is defined in `playwright.config.ts`:

- **Base URL:** http://localhost:3001
- **Timeout:** 30 seconds per test
- **Browsers:** Chromium (add Firefox/WebKit in config if needed)
- **Screenshots:** On failure only
- **Videos:** Retain on failure
- **Reports:** HTML report in `playwright-report/`

### Running Against Production

To run tests against production:
```bash
npx playwright test --baseURL=https://mimir.asgard.internal
```

Note: Requires valid authentication tokens in cookies.

## Debugging

**View test report:**
```bash
npx playwright show-report
```

**Run single test in headed mode:**
```bash
npx playwright test e2e/ocr-annotation.spec.ts:60 --headed
```

**Enable trace viewer:**
```bash
npx playwright test --trace=on
```

Then view trace:
```bash
npx playwright show-trace trace.zip
```

## Continuous Integration

Tests run on CI with retries and parallel execution:
- Retries: 2 attempts
- Workers: 1 (sequential to avoid race conditions)
- On failure: Screenshot + trace captured
- Report: HTML report available as artifact

## Common Issues

**Tests timing out:**
- Ensure dev server is running: `npm run dev`
- Check network connectivity
- Verify IMAGE_BASE_PATH is accessible

**Images not loading:**
- Verify IMAGE_BASE_PATH environment variable is set
- Check that test images exist in /data/images/
- Review browser console for 404 errors

**Dataset list empty:**
- Run annotation benchmark first to create tasks
- Verify database has ocr_eval_datasets and ocr_annotation_tasks rows
- Check tenant isolation (tenant_id in cookies)

**Status not updating:**
- Verify backend API is responding
- Check database connection
- Review browser network tab for failed requests
