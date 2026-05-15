#!/usr/bin/env node
/**
 * RAG Studio Test Suite for asgard-medical Tenant
 *
 * Tests the RAG Playground functionality on the medical tenant:
 * - Health checks
 * - Tenant verification
 * - Document ingestion
 * - Vector search
 * - Hybrid search
 * - Query responses
 *
 * Usage:
 *   node test_rag_playground_medical.js
 *   MIMIR_URL=http://localhost:3002 node test_rag_playground_medical.js
 */

const BASE_URL = process.env.MIMIR_URL || 'http://localhost:3002';
const TENANT_ID = 'asgard-medical';
const TEST_TIMEOUT = 30000;

let testsPassed = 0;
let testsFailed = 0;

// Color codes for terminal output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m'
};

async function log(message, type = 'info') {
  const timestamp = new Date().toISOString();
  const color = type === 'pass' ? colors.green :
                type === 'fail' ? colors.red :
                type === 'warn' ? colors.yellow :
                type === 'section' ? colors.cyan : colors.blue;
  console.log(`${color}[${timestamp}]${colors.reset} ${message}`);
}

async function testHealthCheck() {
  try {
    const res = await fetch(`${BASE_URL}/health`, { timeout: TEST_TIMEOUT });
    const data = await res.json();

    if (res.status === 200 && data.status === 'ok') {
      await log('✓ Health check passed', 'pass');
      testsPassed++;
    } else {
      await log(`✗ Health check failed: ${res.status}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Health check error: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testTenantExists() {
  try {
    const res = await fetch(`${BASE_URL}/api/v1/tenants/${TENANT_ID}`, {
      timeout: TEST_TIMEOUT
    });

    if (res.status === 200) {
      const data = await res.json();
      await log(`✓ Tenant '${TENANT_ID}' exists (Domain: ${data.domain})`, 'pass');
      testsPassed++;
    } else if (res.status === 404) {
      await log(`✗ Tenant '${TENANT_ID}' not found (404)`, 'fail');
      testsFailed++;
    } else {
      await log(`✗ Unexpected status ${res.status} when checking tenant`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error checking tenant: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testListDocuments() {
  try {
    const res = await fetch(
      `${BASE_URL}/api/v1/tenants/${TENANT_ID}/ingest/documents`,
      { timeout: TEST_TIMEOUT }
    );

    if (res.status === 200) {
      const documents = await res.json();
      await log(`✓ Documents listed: ${documents.length} documents found`, 'pass');
      if (documents.length > 0) {
        documents.slice(0, 3).forEach(doc => {
          console.log(`  - "${doc.title}" (ID: ${doc.id})`);
        });
      }
      testsPassed++;
    } else {
      await log(`✗ Failed to list documents: ${res.status}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error listing documents: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testIngestDocument() {
  try {
    const testDoc = {
      title: "RAG Playground Test Document",
      content: `# Medical Knowledge Test Document

## Symptoms and Signs
- Fever: elevated body temperature above 38°C
- Cough: persistent, dry or productive
- Shortness of breath: dyspnea, difficulty breathing

## Common Diagnoses
- Pneumonia: infection of the lung alveoli
- Bronchitis: inflammation of the bronchial tubes
- COVID-19: coronavirus disease 2019

## Recommended Tests
- Chest X-ray: for respiratory imaging
- Blood culture: for bacterial infection detection
- PCR test: for viral pathogen identification

## Treatment Guidelines
- Supportive care: rest, hydration, monitoring
- Antibiotics: if bacterial infection confirmed
- Antiviral agents: for viral infections

## References
Based on standard medical protocols and evidence-based medicine.`,
      source: "rag-playground-test"
    };

    const res = await fetch(
      `${BASE_URL}/api/v1/tenants/${TENANT_ID}/ingest`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(testDoc),
        timeout: TEST_TIMEOUT
      }
    );

    if (res.status === 200) {
      const result = await res.json();
      await log(
        `✓ Document ingested (ID: ${result.document_id}, Status: ${result.status})`,
        'pass'
      );
      testsPassed++;
    } else {
      const errorText = await res.text();
      await log(`✗ Document ingestion failed: ${res.status} - ${errorText}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error ingesting document: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testVectorSearch() {
  try {
    const query = {
      question: "What are the symptoms of respiratory infections?",
      mode: "vector"
    };

    const res = await fetch(
      `${BASE_URL}/api/v1/tenants/${TENANT_ID}/query`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(query),
        timeout: TEST_TIMEOUT
      }
    );

    if (res.status === 200) {
      const result = await res.json();
      await log(
        `✓ Vector search successful (Mode: ${result.mode_used}, Sources: ${result.sources?.length || 0})`,
        'pass'
      );

      if (result.answer) {
        console.log(`\n  Answer Preview:\n  "${result.answer.substring(0, 120)}..."\n`);
      }
      testsPassed++;
    } else {
      const errorText = await res.text();
      await log(`✗ Vector search failed: ${res.status}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error in vector search: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testHybridSearch() {
  try {
    const query = {
      question: "What treatment options are available for respiratory infections?",
      mode: "hybrid"
    };

    const res = await fetch(
      `${BASE_URL}/api/v1/tenants/${TENANT_ID}/query`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(query),
        timeout: TEST_TIMEOUT
      }
    );

    if (res.status === 200) {
      const result = await res.json();
      await log(
        `✓ Hybrid search successful (Mode: ${result.mode_used}, Sources: ${result.sources?.length || 0})`,
        'pass'
      );
      testsPassed++;
    } else {
      const errorText = await res.text();
      await log(`✗ Hybrid search failed: ${res.status}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error in hybrid search: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function testEmptyQuery() {
  try {
    const query = {
      question: "General medical information about common diseases"
    };

    const res = await fetch(
      `${BASE_URL}/api/v1/tenants/${TENANT_ID}/query`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(query),
        timeout: TEST_TIMEOUT
      }
    );

    if (res.status === 200) {
      const result = await res.json();
      await log(
        `✓ General query successful (Sources: ${result.sources?.length || 0})`,
        'pass'
      );
      testsPassed++;
    } else {
      await log(`✗ General query failed: ${res.status}`, 'fail');
      testsFailed++;
    }
  } catch (error) {
    await log(`✗ Error in general query: ${error.message}`, 'fail');
    testsFailed++;
  }
}

async function runAllTests() {
  await log('=== RAG Studio Test Suite for asgard-medical ===', 'section');
  await log(`Base URL: ${BASE_URL}`, 'info');
  await log(`Tenant: ${TENANT_ID}\n`, 'info');

  // Phase 1: Infrastructure
  await log('Phase 1: Infrastructure & Connectivity', 'section');
  await testHealthCheck();

  // Phase 2: Tenant Verification
  await log('\nPhase 2: Tenant Verification', 'section');
  await testTenantExists();

  // Phase 3: Document Management
  await log('\nPhase 3: Document Management', 'section');
  await testListDocuments();
  await testIngestDocument();

  // Phase 4: RAG Queries
  await log('\nPhase 4: RAG Queries', 'section');
  await testVectorSearch();
  await testHybridSearch();
  await testEmptyQuery();

  // Summary
  await log('\n=== Test Summary ===', 'section');
  const total = testsPassed + testsFailed;
  const percentage = total > 0 ? Math.round((testsPassed / total) * 100) : 0;

  const summaryColor = testsFailed === 0 ? colors.green :
                       testsFailed <= 2 ? colors.yellow : colors.red;

  console.log(`${summaryColor}
Passed:  ${testsPassed}/${total}
Failed:  ${testsFailed}/${total}
Success: ${percentage}%
${colors.reset}`);

  process.exit(testsFailed > 0 ? 1 : 0);
}

// Handle graceful shutdown
process.on('SIGINT', async () => {
  await log('\n\nTest run interrupted', 'warn');
  process.exit(1);
});

// Run tests
runAllTests().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});
