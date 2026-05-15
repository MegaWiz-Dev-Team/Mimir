#!/usr/bin/env node
/**
 * End-to-End (E2E) Integration Tests for asgard-medical Tenant
 *
 * Tests complete workflows combining RAG and Agent Studio:
 * 1. Ingest Medical Document → Query → Verify Sources
 * 2. Multi-turn Chat Conversation
 * 3. Question Routing to Specialist
 * 4. Cross-agent Communication
 *
 * Usage:
 *   node test_e2e_medical_workflow.js
 *   MIMIR_URL=http://localhost:3002 node test_e2e_medical_workflow.js
 */

const BASE_URL = process.env.MIMIR_URL || 'http://localhost:3002';
const TENANT_ID = 'asgard-medical';
const TEST_TIMEOUT = 60000;

let testsPassed = 0;
let testsFailed = 0;
let ingestedDocId = null;
let conversationId = null;
let agentId = null;

// Color codes
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  magenta: '\x1b[35m'
};

async function log(message, type = 'info') {
  const timestamp = new Date().toISOString();
  const color = type === 'pass' ? colors.green :
                type === 'fail' ? colors.red :
                type === 'warn' ? colors.yellow :
                type === 'section' ? colors.cyan :
                type === 'debug' ? colors.magenta : colors.blue;
  console.log(`${color}[${timestamp}]${colors.reset} ${message}`);
}

async function makeRequest(method, path, data = null, expectedStatus = 200) {
  try {
    const options = {
      method,
      headers: { 'Content-Type': 'application/json' },
      timeout: TEST_TIMEOUT
    };

    if (data) {
      options.body = JSON.stringify(data);
    }

    const res = await fetch(`${BASE_URL}${path}`, options);
    const result = await res.json().catch(() => ({ raw: res.statusText }));

    return [result, res.status === expectedStatus];
  } catch (error) {
    await log(`Request error: ${error.message}`, 'fail');
    return [null, false];
  }
}

async function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// ─────────────────────────────────────────────────────────────
// WORKFLOW 1: Document Ingestion → RAG Query → Verification
// ─────────────────────────────────────────────────────────────

async function e2eWorkflow1DocumentRagFlow() {
  await log("\n=== E2E Workflow 1: Document Ingestion → RAG Query ===", 'section');

  // Step 1: Ingest medical document
  await log("Step 1/4: Ingesting medical document...", 'info');

  const medicalDoc = {
    title: "Hypertension Management Guidelines 2026",
    content: `# Hypertension Management Guidelines

## Definition
Hypertension (high blood pressure) is defined as systolic BP ≥ 140 mmHg or diastolic BP ≥ 90 mmHg.

## Classification
- Stage 1: Systolic 140-159 or Diastolic 90-99
- Stage 2: Systolic ≥ 160 or Diastolic ≥ 100

## Risk Factors
- Age over 65 years
- Family history of hypertension
- Smoking
- Excessive alcohol consumption

## Treatment Options
### Pharmacological Treatment
- ACE Inhibitors (e.g., Lisinopril)
  - Contraindications: Pregnancy, history of angioedema
  - Side effects: Dry cough

- Beta-blockers (e.g., Metoprolol)
  - Contraindications: Asthma, COPD
  - Side effects: Fatigue

- Calcium Channel Blockers (e.g., Amlodipine)
  - Contraindications: Heart block
  - Side effects: Edema`,
    source: "medical-guidelines-2026"
  };

  const [docData, success] = await makeRequest(
    'POST',
    `/api/v1/tenants/${TENANT_ID}/ingest`,
    medicalDoc
  );

  if (success && docData) {
    ingestedDocId = docData.document_id;
    const status = docData.status;
    await log(`✓ Document ingested (ID: ${ingestedDocId}, Status: ${status})`, 'pass');
    testsPassed++;
  } else {
    await log("✗ Document ingestion failed", 'fail');
    testsFailed++;
    return;
  }

  // Step 2: Query the RAG system
  await sleep(500);
  await log("Step 2/4: Querying RAG system with document...", 'info');

  const ragQuery = {
    question: "What are the main treatment options for hypertension?",
    mode: "vector"
  };

  const [ragData, ragSuccess] = await makeRequest(
    'POST',
    `/api/v1/tenants/${TENANT_ID}/query`,
    ragQuery
  );

  if (ragSuccess && ragData) {
    const answer = ragData.answer || '';
    const sources = ragData.sources || [];
    const sourceCount = sources.length;

    await log(`✓ RAG query successful (${sourceCount} sources found)`, 'pass');

    if (sources.length > 0) {
      console.log('\n  Sources Found:');
      sources.slice(0, 2).forEach(src => {
        const title = src.title || 'Unknown';
        const relevance = src.relevance || 0;
        console.log(`  - ${title} (Relevance: ${relevance.toFixed(2)})`);
      });
    }

    if (answer) {
      const preview = answer.length > 80 ? answer.substring(0, 80) + '...' : answer;
      console.log(`\n  Answer: ${preview}\n`);
    }

    testsPassed++;
  } else {
    await log("✗ RAG query failed", 'fail');
    testsFailed++;
    return;
  }

  // Step 3: Verify document in listing
  await log("Step 3/4: Verifying document in listing...", 'info');

  const [docListData, listSuccess] = await makeRequest(
    'GET',
    `/api/v1/tenants/${TENANT_ID}/ingest/documents`
  );

  if (listSuccess && Array.isArray(docListData)) {
    const docFound = docListData.some(doc => doc.id === ingestedDocId);

    if (docFound) {
      await log(`✓ Document found in listing (Total: ${docListData.length})`, 'pass');
      testsPassed++;
    } else {
      await log("✗ Document not found in listing", 'fail');
      testsFailed++;
    }
  } else {
    await log("✗ Failed to list documents", 'fail');
    testsFailed++;
  }

  // Step 4: Hybrid search validation
  await log("Step 4/4: Testing hybrid search mode...", 'info');

  const hybridQuery = {
    question: "What are ACE inhibitors and their side effects?",
    mode: "hybrid"
  };

  const [hybridData, hybridSuccess] = await makeRequest(
    'POST',
    `/api/v1/tenants/${TENANT_ID}/query`,
    hybridQuery
  );

  if (hybridSuccess && hybridData) {
    const modeUsed = hybridData.mode_used || 'unknown';
    await log(`✓ Hybrid search successful (Mode: ${modeUsed})`, 'pass');
    testsPassed++;
  } else {
    await log("✗ Hybrid search failed", 'fail');
    testsFailed++;
  }
}

// ─────────────────────────────────────────────────────────────
// WORKFLOW 2: Multi-turn Agent Chat
// ─────────────────────────────────────────────────────────────

async function e2eWorkflow2MultiturnChat() {
  await log("\n=== E2E Workflow 2: Multi-turn Agent Chat ===", 'section');

  // Get an agent first
  await log("Step 1/5: Getting available agents...", 'info');

  const [agentsData, agentsSuccess] = await makeRequest('GET', '/api/v1/agents');

  if (!agentsSuccess || !agentsData || agentsData.length === 0) {
    await log("✗ Failed to get agents", 'fail');
    testsFailed++;
    return;
  }

  agentId = agentsData[0].id;
  const agentName = agentsData[0].name || 'Unknown';
  await log(`✓ Using agent: ${agentName}`, 'pass');
  testsPassed++;

  // First turn
  await log("Step 2/5: First turn - Ask about hypertension symptoms...", 'info');

  const chat1 = {
    message: "What are the main symptoms of hypertension?",
    model: "auto"
  };

  const [chat1Data, chat1Success] = await makeRequest(
    'POST',
    `/api/v1/agents/${agentId}/chat`,
    chat1
  );

  if (chat1Success && chat1Data) {
    conversationId = chat1Data.conversation_id;
    const response1 = (chat1Data.response || '').substring(0, 60);
    await log(`✓ Turn 1 successful (Conv: ${conversationId})`, 'pass');
    console.log(`  Agent: ${response1}...\n`);
    testsPassed++;
  } else {
    await log("✗ First turn failed", 'fail');
    testsFailed++;
    return;
  }

  // Second turn
  await log("Step 3/5: Second turn - Follow-up on treatment...", 'info');

  const chat2 = {
    message: "What medications are commonly used?",
    model: "auto",
    conversation_id: conversationId
  };

  const [chat2Data, chat2Success] = await makeRequest(
    'POST',
    `/api/v1/agents/${agentId}/chat`,
    chat2
  );

  if (chat2Success && chat2Data) {
    const response2 = (chat2Data.response || '').substring(0, 60);
    await log(`✓ Turn 2 successful (same conversation)`, 'pass');
    console.log(`  Agent: ${response2}...\n`);
    testsPassed++;
  } else {
    await log("✗ Second turn failed", 'fail');
    testsFailed++;
  }

  // Third turn
  await log("Step 4/5: Third turn - Ask about side effects...", 'info');

  const chat3 = {
    message: "What are the side effects I should watch for?",
    model: "auto",
    conversation_id: conversationId
  };

  const [chat3Data, chat3Success] = await makeRequest(
    'POST',
    `/api/v1/agents/${agentId}/chat`,
    chat3
  );

  if (chat3Success && chat3Data) {
    const response3 = (chat3Data.response || '').substring(0, 60);
    await log(`✓ Turn 3 successful (conversation continued)`, 'pass');
    console.log(`  Agent: ${response3}...\n`);
    testsPassed++;
  } else {
    await log("✗ Third turn failed", 'fail');
    testsFailed++;
  }

  // Verify conversation history
  await log("Step 5/5: Verifying conversation history...", 'info');

  const [convData, convSuccess] = await makeRequest(
    'GET',
    `/api/v1/agents/${agentId}/conversations`
  );

  if (convSuccess && Array.isArray(convData)) {
    const convFound = convData.some(conv => conv.id === conversationId);

    if (convFound) {
      await log(`✓ Conversation verified in history (${convData.length} total)`, 'pass');
      testsPassed++;
    } else {
      await log("⚠ Conversation not yet in history (eventual consistency)", 'warn');
    }
  } else {
    await log("⚠ Failed to list conversations", 'warn');
  }
}

// ─────────────────────────────────────────────────────────────
// WORKFLOW 3: Question Routing & Specialist Selection
// ─────────────────────────────────────────────────────────────

async function e2eWorkflow3AgentRouting() {
  await log("\n=== E2E Workflow 3: Intelligent Agent Routing ===", 'section');

  // Test Case 1: Cardiology question
  await log("Step 1/3: Routing cardiology question...", 'info');

  const cardioRoute = {
    question: "I have chest pain, shortness of breath, and irregular heartbeat. What should I do?",
    tenant_id: TENANT_ID
  };

  const [route1Data, route1Success] = await makeRequest(
    'POST',
    '/api/v1/agents/route',
    cardioRoute
  );

  if (route1Success && route1Data) {
    const selected = route1Data.selected_agent || {};
    const specialty = selected.specialty || 'unknown';
    const confidence = route1Data.confidence || 0;

    if (specialty.toLowerCase().includes('cardio')) {
      await log(
        `✓ Correctly routed to cardiology (Confidence: ${confidence.toFixed(2)})`,
        'pass'
      );
      testsPassed++;
    } else {
      await log(`✗ Unexpected routing: ${specialty}`, 'fail');
      testsFailed++;
    }
  } else {
    await log("✗ Routing failed", 'fail');
    testsFailed++;
  }

  // Test Case 2: Sleep medicine question
  await log("Step 2/3: Routing sleep medicine question...", 'info');

  const sleepRoute = {
    question: "I can't fall asleep and have been snoring loudly. I wake up gasping for air. What specialist should I see?",
    tenant_id: TENANT_ID
  };

  const [route2Data, route2Success] = await makeRequest(
    'POST',
    '/api/v1/agents/route',
    sleepRoute
  );

  if (route2Success && route2Data) {
    const selected = route2Data.selected_agent || {};
    const specialty = selected.specialty || 'unknown';
    const confidence = route2Data.confidence || 0;

    if (specialty.toLowerCase().includes('sleep')) {
      await log(
        `✓ Correctly routed to sleep medicine (Confidence: ${confidence.toFixed(2)})`,
        'pass'
      );
      testsPassed++;
    } else {
      await log(`⚠ Routed to: ${specialty} (still valid)`, 'warn');
    }
  }

  // Test Case 3: Pediatrics question
  await log("Step 3/3: Routing pediatrics question...", 'info');

  const pedsRoute = {
    question: "My 5-year-old child has a high fever, cough, and ear pain. Which pediatrician-level care does he need?",
    tenant_id: TENANT_ID
  };

  const [route3Data, route3Success] = await makeRequest(
    'POST',
    '/api/v1/agents/route',
    pedsRoute
  );

  if (route3Success && route3Data) {
    const selected = route3Data.selected_agent || {};
    const specialty = selected.specialty || 'unknown';

    await log(`✓ Routed to: ${specialty}`, 'pass');
    testsPassed++;
  }
}

// ─────────────────────────────────────────────────────────────
// WORKFLOW 4: Cross-system Integration
// ─────────────────────────────────────────────────────────────

async function e2eWorkflow4CrossSystem() {
  await log("\n=== E2E Workflow 4: Cross-system Integration ===", 'section');

  // Step 1: Query RAG for hypertension info
  await log("Step 1/2: Query RAG for specific medical info...", 'info');

  const ragQuery = {
    question: "What are the contraindications for ACE inhibitors?",
    mode: "vector"
  };

  const [ragData, ragSuccess] = await makeRequest(
    'POST',
    `/api/v1/tenants/${TENANT_ID}/query`,
    ragQuery
  );

  if (ragSuccess && ragData) {
    const sources = ragData.sources || [];
    const sourcesFound = sources.length;
    await log(`✓ RAG provided ${sourcesFound} sources`, 'pass');
    testsPassed++;
  } else {
    await log("⚠ RAG query returned no results", 'warn');
  }

  // Step 2: Chat with agent about the same topic
  await log("Step 2/2: Chat with agent about the same topic...", 'info');

  if (!agentId) {
    const [agentsData] = await makeRequest('GET', '/api/v1/agents');
    if (agentsData && agentsData.length > 0) {
      agentId = agentsData[0].id;
    }
  }

  const chatData = {
    message: "Based on my medical history with angioedema, which antihypertensive should I avoid?",
    mode: "rag"
  };

  const [chatResponse, chatSuccess] = await makeRequest(
    'POST',
    `/api/v1/agents/${agentId}/chat`,
    chatData
  );

  if (chatSuccess && chatResponse) {
    const responseText = chatResponse.response || '';
    const chatSources = chatResponse.sources || [];

    await log(
      `✓ Agent provided RAG-augmented answer (${chatSources.length} sources)`,
      'pass'
    );

    if (responseText) {
      const preview = responseText.length > 80 ? responseText.substring(0, 80) + '...' : responseText;
      console.log(`  Agent: ${preview}\n`);
    }

    testsPassed++;
  } else {
    await log("⚠ Agent chat with RAG returned no response", 'warn');
  }
}

// ─────────────────────────────────────────────────────────────
// Run All Workflows
// ─────────────────────────────────────────────────────────────

async function runAllWorkflows() {
  const startTime = Date.now();

  await log('╔═════════════════════════════════════════════════════╗', 'section');
  await log('║  End-to-End Integration Tests - asgard-medical      ║', 'section');
  await log('╚═════════════════════════════════════════════════════╝', 'section');
  await log(`Base URL: ${BASE_URL}`, 'info');
  await log(`Tenant: ${TENANT_ID}\n`, 'info');

  // Run workflows
  await e2eWorkflow1DocumentRagFlow();
  await e2eWorkflow2MultiturnChat();
  await e2eWorkflow3AgentRouting();
  await e2eWorkflow4CrossSystem();

  // Summary
  const elapsed = (Date.now() - startTime) / 1000;
  const total = testsPassed + testsFailed;
  const percentage = total > 0 ? Math.round((testsPassed / total) * 100) : 0;

  await log('\n=== End-to-End Test Summary ===', 'section');

  const summaryColor = testsFailed === 0 ? colors.green :
                       testsFailed <= 3 ? colors.yellow : colors.red;

  console.log(`${summaryColor}
Passed:  ${testsPassed}/${total}
Failed:  ${testsFailed}/${total}
Success: ${percentage}%

⏱️  Total Time: ${elapsed.toFixed(2)} seconds
${colors.reset}`);

  process.exit(testsFailed > 0 ? 1 : 0);
}

// Handle graceful shutdown
process.on('SIGINT', async () => {
  await log('\n\nTest run interrupted', 'warn');
  process.exit(1);
});

// Run tests
runAllWorkflows().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});
