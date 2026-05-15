#!/usr/bin/env node
/**
 * Agent Studio API Test Suite for asgard-medical Tenant
 *
 * Tests agent management and chat functionality:
 * - List all agents
 * - Get agent templates
 * - Retrieve specific agent config
 * - Chat with an agent
 * - Conversation management
 * - Agent routing
 *
 * Usage:
 *   node test_agents_api_medical.js
 *   MIMIR_URL=http://localhost:3002 node test_agents_api_medical.js
 */

const BASE_URL = process.env.MIMIR_URL || 'http://localhost:3002';
const TENANT_ID = 'asgard-medical';
const TEST_TIMEOUT = 30000;

let testsPassed = 0;
let testsFailed = 0;
let agents = [];

// Color codes for terminal output
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

async function testListAgents() {
  const [data, success] = await makeRequest('GET', '/api/v1/agents');

  if (success && Array.isArray(data)) {
    agents = data;
    await log(`✓ Found ${agents.length} agents`, 'pass');

    console.log(`\n  ${'Agent ID':<25} ${'Name':<30} ${'Specialty':<20} ${'Model':<30}`);
    console.log(`  ${'-'.repeat(25)} ${'-'.repeat(30)} ${'-'.repeat(20)} ${'-'.repeat(30)}`);

    agents.forEach(agent => {
      const agentId = (agent.id || 'N/A').substring(0, 25).padEnd(25);
      const name = (agent.name || 'Unnamed').substring(0, 30).padEnd(30);
      const specialty = (agent.specialty || 'General').substring(0, 20).padEnd(20);
      const model = (agent.model_id || 'N/A').substring(0, 30).padEnd(30);
      console.log(`  ${agentId} ${name} ${specialty} ${model}`);
    });

    testsPassed++;
  } else {
    await log('✗ Failed to list agents', 'fail');
    testsFailed++;
  }
}

async function testGetAgentTemplates() {
  const [data, success] = await makeRequest('GET', '/api/v1/agents/templates');

  if (success && Array.isArray(data)) {
    await log(`✓ Found ${data.length} agent templates`, 'pass');

    data.slice(0, 3).forEach(template => {
      const desc = (template.description || '').substring(0, 60);
      console.log(`  - ${template.name}: ${desc}...`);
    });

    testsPassed++;
  } else {
    await log('✗ Failed to get templates', 'fail');
    testsFailed++;
  }
}

async function testGetSpecificAgent() {
  if (agents.length === 0) {
    await log('⊘ Skipping: No agents available', 'warn');
    return;
  }

  const agentId = agents[0].id;
  const [data, success] = await makeRequest('GET', `/api/v1/agents/${agentId}`);

  if (success && data) {
    const name = data.name || 'Unknown';
    const specialty = data.specialty || 'General';
    const useRag = data.use_rag || false;
    const temperature = data.temperature || 0;

    await log(
      `✓ Retrieved agent '${name}' (RAG: ${useRag}, Temp: ${temperature})`,
      'pass'
    );
    testsPassed++;
  } else {
    await log('✗ Failed to get agent details', 'fail');
    testsFailed++;
  }
}

async function testAgentChat() {
  if (agents.length === 0) {
    await log('⊘ Skipping: No agents available', 'warn');
    return;
  }

  const agentId = agents[0].id;
  const chatRequest = {
    message: 'What are common symptoms of respiratory infections?',
    model: 'auto'
  };

  const [data, success] = await makeRequest(
    'POST',
    `/api/v1/agents/${agentId}/chat`,
    chatRequest
  );

  if (success && data) {
    const response = data.response || '';
    const conversationId = data.conversation_id || 'N/A';

    if (response) {
      const preview = response.length > 100 ? response.substring(0, 100) + '...' : response;
      await log(
        `✓ Chat successful (Conv: ${conversationId})`,
        'pass'
      );
      console.log(`\n  Response Preview:\n  "${preview}"\n`);
      testsPassed++;
    } else {
      await log('✗ No response from agent', 'fail');
      testsFailed++;
    }
  } else {
    await log('✗ Chat request failed', 'fail');
    testsFailed++;
  }
}

async function testAgentConversations() {
  if (agents.length === 0) {
    await log('⊘ Skipping: No agents available', 'warn');
    return;
  }

  const agentId = agents[0].id;
  const [data, success] = await makeRequest('GET', `/api/v1/agents/${agentId}/conversations`);

  if (success && Array.isArray(data)) {
    await log(
      `✓ Retrieved ${data.length} conversations for agent`,
      'pass'
    );

    data.slice(0, 3).forEach(conv => {
      const convId = conv.id || 'N/A';
      const created = conv.created_at || 'N/A';
      console.log(`  - Conversation ${convId} (Created: ${created})`);
    });

    testsPassed++;
  } else {
    await log('✗ Failed to list conversations', 'fail');
    testsFailed++;
  }
}

async function testAgentRouting() {
  const routeRequest = {
    question: 'I have chest pain and difficulty breathing. What specialist should I see?',
    tenant_id: TENANT_ID
  };

  const [data, success] = await makeRequest(
    'POST',
    '/api/v1/agents/route',
    routeRequest
  );

  if (success && data) {
    const selectedAgent = data.selected_agent || {};
    const agentName = selectedAgent.name || 'Unknown';
    const specialty = selectedAgent.specialty || 'Unknown';
    const confidence = data.confidence || 0;

    await log(
      `✓ Routed to specialist: '${agentName}' (${specialty}) - Confidence: ${confidence.toFixed(2)}`,
      'pass'
    );
    testsPassed++;
  } else {
    await log('✗ Agent routing failed', 'fail');
    testsFailed++;
  }
}

async function testAgentListQuery() {
  const [data, success] = await makeRequest(
    'GET',
    '/api/v1/agents?specialty=cardiology&limit=5'
  );

  if (success && Array.isArray(data)) {
    await log(
      `✓ Filtered agents by specialty: ${data.length} results`,
      'pass'
    );
    testsPassed++;
  } else {
    await log('✗ Failed to filter agents', 'fail');
    testsFailed++;
  }
}

async function runAllTests() {
  await log('=== Agent Studio API Test Suite ===', 'section');
  await log(`Base URL: ${BASE_URL}`, 'info');
  await log(`Tenant: ${TENANT_ID}\n`, 'info');

  // Phase 1: Agent Discovery
  await log('Phase 1: Agent Discovery', 'section');
  await testListAgents();
  await testGetAgentTemplates();

  // Phase 2: Agent Management
  await log('\nPhase 2: Agent Management', 'section');
  await testGetSpecificAgent();

  // Phase 3: Agent Chat
  await log('\nPhase 3: Agent Chat & Conversations', 'section');
  await testAgentChat();
  await testAgentConversations();

  // Phase 4: Advanced Features
  await log('\nPhase 4: Advanced Features', 'section');
  await testAgentRouting();
  await testAgentListQuery();

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
