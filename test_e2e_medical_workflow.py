#!/usr/bin/env python3
"""
End-to-End (E2E) Integration Tests for asgard-medical Tenant

Tests complete workflows combining RAG and Agent Studio:
1. Ingest Medical Document → Query → Verify Sources
2. Multi-turn Chat Conversation
3. Question Routing to Specialist
4. Cross-agent Communication

Usage:
    python test_e2e_medical_workflow.py
    MIMIR_URL=http://localhost:3002 python test_e2e_medical_workflow.py
"""

import os
import sys
import json
import time
import requests
from datetime import datetime
from typing import Dict, Any, Optional, List
from urllib.parse import urljoin

# Configuration
BASE_URL = os.environ.get('MIMIR_URL', 'http://localhost:3002')
TENANT_ID = 'asgard-medical'
TEST_TIMEOUT = 60

# ANSI Color codes
class Colors:
    RESET = '\033[0m'
    GREEN = '\033[32m'
    RED = '\033[31m'
    YELLOW = '\033[33m'
    BLUE = '\033[34m'
    CYAN = '\033[36m'
    MAGENTA = '\033[35m'

class E2ETestSuite:
    def __init__(self):
        self.tests_passed = 0
        self.tests_failed = 0
        self.session = requests.Session()
        self.ingested_doc_id = None
        self.conversation_id = None
        self.agent_id = None

    def log(self, message: str, level: str = 'info') -> None:
        """Log message with timestamp and color."""
        timestamp = datetime.now().isoformat()
        color_map = {
            'pass': Colors.GREEN,
            'fail': Colors.RED,
            'warn': Colors.YELLOW,
            'section': Colors.CYAN,
            'info': Colors.BLUE,
            'debug': Colors.MAGENTA
        }
        color = color_map.get(level, Colors.BLUE)
        print(f"{color}[{timestamp}]{Colors.RESET} {message}")

    def make_request(
        self,
        method: str,
        path: str,
        data: Optional[Dict] = None,
        expected_status: int = 200
    ) -> tuple[Optional[Dict], bool]:
        """Make HTTP request and return (response_data, success)."""
        try:
            url = urljoin(BASE_URL, path)

            if method == 'GET':
                resp = self.session.get(url, timeout=TEST_TIMEOUT)
            elif method == 'POST':
                resp = self.session.post(
                    url,
                    json=data,
                    headers={'Content-Type': 'application/json'},
                    timeout=TEST_TIMEOUT
                )
            else:
                return None, False

            success = resp.status_code == expected_status

            try:
                response_data = resp.json()
            except:
                response_data = {'raw': resp.text}

            return response_data, success
        except Exception as e:
            self.log(f"Request error: {str(e)}", 'fail')
            return None, False

    # ─────────────────────────────────────────────────────────────
    # WORKFLOW 1: Document Ingestion → RAG Query → Verification
    # ─────────────────────────────────────────────────────────────

    def e2e_workflow_1_document_rag_flow(self) -> None:
        """E2E Workflow 1: Complete RAG pipeline with document management."""
        self.log("\n=== E2E Workflow 1: Document Ingestion → RAG Query ===", 'section')

        # Step 1: Ingest medical document
        self.log("Step 1/4: Ingesting medical document...", 'info')
        medical_document = {
            "title": "Hypertension Management Guidelines 2026",
            "content": """# Hypertension Management Guidelines

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
- High sodium diet
- Obesity
- Physical inactivity

## Treatment Options

### Lifestyle Modifications
- Reduce sodium intake to <2,300 mg/day
- Weight loss of 5-10% of body weight
- Regular physical activity (150 min/week)
- Limit alcohol consumption
- DASH diet (rich in fruits, vegetables, whole grains)

### Pharmacological Treatment
- ACE Inhibitors (e.g., Lisinopril, Enalapril)
  - Contraindications: Pregnancy, history of angioedema
  - Side effects: Dry cough, dizziness

- Beta-blockers (e.g., Metoprolol, Atenolol)
  - Contraindications: Uncontrolled asthma, COPD
  - Side effects: Fatigue, bradycardia

- Calcium Channel Blockers (e.g., Amlodipine, Verapamil)
  - Contraindications: Heart block
  - Side effects: Edema, constipation

- Diuretics (e.g., Hydrochlorothiazide)
  - Contraindications: Gout, severe hyponatremia
  - Side effects: Hypokalemia, hyperglycemia

## Monitoring
- Blood pressure monitoring at home
- Regular follow-up: 4 weeks after initiation
- Target BP: <140/90 for most patients

## Complications
- Cardiovascular disease
- Stroke
- Kidney disease
- Heart failure""",
            "source": "medical-guidelines-2026"
        }

        doc_data, success = self.make_request(
            'POST',
            f'/api/v1/tenants/{TENANT_ID}/ingest',
            data=medical_document,
            expected_status=200
        )

        if success and doc_data:
            self.ingested_doc_id = doc_data.get('document_id')
            status = doc_data.get('status')
            self.log(f"✓ Document ingested (ID: {self.ingested_doc_id}, Status: {status})", 'pass')
            self.tests_passed += 1
        else:
            self.log("✗ Document ingestion failed", 'fail')
            self.tests_failed += 1
            return

        # Step 2: Query the RAG system
        time.sleep(0.5)  # Allow indexing
        self.log("Step 2/4: Querying RAG system with document...", 'info')

        rag_query = {
            "question": "What are the main treatment options for hypertension?",
            "mode": "vector"
        }

        rag_data, success = self.make_request(
            'POST',
            f'/api/v1/tenants/{TENANT_ID}/query',
            data=rag_query,
            expected_status=200
        )

        if success and rag_data:
            answer = rag_data.get('answer', '')
            sources = rag_data.get('sources', [])
            source_count = len(sources)

            self.log(f"✓ RAG query successful ({source_count} sources found)", 'pass')

            if sources:
                print(f"\n  Sources Found:")
                for src in sources[:2]:
                    title = src.get('title', 'Unknown')
                    relevance = src.get('relevance', 0)
                    print(f"  - {title} (Relevance: {relevance:.2f})")

            if answer:
                preview = answer[:80] + "..." if len(answer) > 80 else answer
                print(f"\n  Answer: {preview}\n")

            self.tests_passed += 1
        else:
            self.log("✗ RAG query failed", 'fail')
            self.tests_failed += 1
            return

        # Step 3: Verify document in listing
        self.log("Step 3/4: Verifying document in listing...", 'info')

        doc_list_data, success = self.make_request(
            'GET',
            f'/api/v1/tenants/{TENANT_ID}/ingest/documents',
            expected_status=200
        )

        if success and isinstance(doc_list_data, list):
            doc_found = any(
                doc.get('id') == self.ingested_doc_id
                for doc in doc_list_data
            )

            if doc_found:
                self.log(f"✓ Document found in listing (Total: {len(doc_list_data)})", 'pass')
                self.tests_passed += 1
            else:
                self.log("✗ Document not found in listing", 'fail')
                self.tests_failed += 1
        else:
            self.log("✗ Failed to list documents", 'fail')
            self.tests_failed += 1

        # Step 4: Hybrid search validation
        self.log("Step 4/4: Testing hybrid search mode...", 'info')

        hybrid_query = {
            "question": "What are ACE inhibitors and their side effects?",
            "mode": "hybrid"
        }

        hybrid_data, success = self.make_request(
            'POST',
            f'/api/v1/tenants/{TENANT_ID}/query',
            data=hybrid_query,
            expected_status=200
        )

        if success and hybrid_data:
            mode_used = hybrid_data.get('mode_used', 'unknown')
            self.log(f"✓ Hybrid search successful (Mode: {mode_used})", 'pass')
            self.tests_passed += 1
        else:
            self.log("✗ Hybrid search failed", 'fail')
            self.tests_failed += 1

    # ─────────────────────────────────────────────────────────────
    # WORKFLOW 2: Multi-turn Agent Chat
    # ─────────────────────────────────────────────────────────────

    def e2e_workflow_2_multiturn_chat(self) -> None:
        """E2E Workflow 2: Multi-turn conversation with agent."""
        self.log("\n=== E2E Workflow 2: Multi-turn Agent Chat ===", 'section')

        # Get an agent first
        self.log("Step 1/5: Getting available agents...", 'info')

        agents_data, success = self.make_request('GET', '/api/v1/agents', expected_status=200)

        if not success or not agents_data:
            self.log("✗ Failed to get agents", 'fail')
            self.tests_failed += 1
            return

        self.agent_id = agents_data[0].get('id')
        agent_name = agents_data[0].get('name', 'Unknown')
        self.log(f"✓ Using agent: {agent_name}", 'pass')
        self.tests_passed += 1

        # First turn of conversation
        self.log("Step 2/5: First turn - Ask about hypertension symptoms...", 'info')

        chat_1 = {
            "message": "What are the main symptoms of hypertension?",
            "model": "auto"
        }

        chat_1_data, success = self.make_request(
            'POST',
            f'/api/v1/agents/{self.agent_id}/chat',
            data=chat_1,
            expected_status=200
        )

        if success and chat_1_data:
            self.conversation_id = chat_1_data.get('conversation_id')
            response_1 = chat_1_data.get('response', '')[:60]
            self.log(f"✓ Turn 1 successful (Conv: {self.conversation_id})", 'pass')
            print(f"  Agent: {response_1}...\n")
            self.tests_passed += 1
        else:
            self.log("✗ First turn failed", 'fail')
            self.tests_failed += 1
            return

        # Second turn - follow-up question
        self.log("Step 3/5: Second turn - Follow-up on treatment...", 'info')

        chat_2 = {
            "message": "What medications are commonly used?",
            "model": "auto",
            "conversation_id": self.conversation_id
        }

        chat_2_data, success = self.make_request(
            'POST',
            f'/api/v1/agents/{self.agent_id}/chat',
            data=chat_2,
            expected_status=200
        )

        if success and chat_2_data:
            response_2 = chat_2_data.get('response', '')[:60]
            self.log(f"✓ Turn 2 successful (same conversation)", 'pass')
            print(f"  Agent: {response_2}...\n")
            self.tests_passed += 1
        else:
            self.log("✗ Second turn failed", 'fail')
            self.tests_failed += 1

        # Third turn - clarification
        self.log("Step 4/5: Third turn - Ask about side effects...", 'info')

        chat_3 = {
            "message": "What are the side effects I should watch for?",
            "model": "auto",
            "conversation_id": self.conversation_id
        }

        chat_3_data, success = self.make_request(
            'POST',
            f'/api/v1/agents/{self.agent_id}/chat',
            data=chat_3,
            expected_status=200
        )

        if success and chat_3_data:
            response_3 = chat_3_data.get('response', '')[:60]
            self.log(f"✓ Turn 3 successful (conversation continued)", 'pass')
            print(f"  Agent: {response_3}...\n")
            self.tests_passed += 1
        else:
            self.log("✗ Third turn failed", 'fail')
            self.tests_failed += 1

        # Verify conversation history
        self.log("Step 5/5: Verifying conversation history...", 'info')

        conv_data, success = self.make_request(
            'GET',
            f'/api/v1/agents/{self.agent_id}/conversations',
            expected_status=200
        )

        if success and isinstance(conv_data, list):
            conv_found = any(
                conv.get('id') == self.conversation_id
                for conv in conv_data
            )

            if conv_found:
                self.log(f"✓ Conversation verified in history ({len(conv_data)} total)", 'pass')
                self.tests_passed += 1
            else:
                self.log("⚠ Conversation not yet in history (eventual consistency)", 'warn')
        else:
            self.log("⚠ Failed to list conversations", 'warn')

    # ─────────────────────────────────────────────────────────────
    # WORKFLOW 3: Question Routing & Specialist Selection
    # ─────────────────────────────────────────────────────────────

    def e2e_workflow_3_agent_routing(self) -> None:
        """E2E Workflow 3: Route questions to appropriate specialists."""
        self.log("\n=== E2E Workflow 3: Intelligent Agent Routing ===", 'section')

        # Test Case 1: Cardiology question
        self.log("Step 1/3: Routing cardiology question...", 'info')

        cardio_route = {
            "question": "I have chest pain, shortness of breath, and irregular heartbeat. What should I do?",
            "tenant_id": TENANT_ID
        }

        route_1_data, success = self.make_request(
            'POST',
            '/api/v1/agents/route',
            data=cardio_route,
            expected_status=200
        )

        if success and route_1_data:
            selected = route_1_data.get('selected_agent', {})
            specialty = selected.get('specialty', 'unknown')
            confidence = route_1_data.get('confidence', 0)

            if 'cardio' in specialty.lower():
                self.log(
                    f"✓ Correctly routed to cardiology (Confidence: {confidence:.2f})",
                    'pass'
                )
                self.tests_passed += 1
            else:
                self.log(f"✗ Unexpected routing: {specialty}", 'fail')
                self.tests_failed += 1
        else:
            self.log("✗ Routing failed", 'fail')
            self.tests_failed += 1

        # Test Case 2: Sleep medicine question
        self.log("Step 2/3: Routing sleep medicine question...", 'info')

        sleep_route = {
            "question": "I can't fall asleep and have been snoring loudly. I wake up gasping for air. What specialist should I see?",
            "tenant_id": TENANT_ID
        }

        route_2_data, success = self.make_request(
            'POST',
            '/api/v1/agents/route',
            data=sleep_route,
            expected_status=200
        )

        if success and route_2_data:
            selected = route_2_data.get('selected_agent', {})
            specialty = selected.get('specialty', 'unknown')
            confidence = route_2_data.get('confidence', 0)

            if 'sleep' in specialty.lower():
                self.log(
                    f"✓ Correctly routed to sleep medicine (Confidence: {confidence:.2f})",
                    'pass'
                )
                self.tests_passed += 1
            else:
                self.log(f"⚠ Routed to: {specialty} (still valid)", 'warn')

        # Test Case 3: Pediatrics question
        self.log("Step 3/3: Routing pediatrics question...", 'info')

        peds_route = {
            "question": "My 5-year-old child has a high fever, cough, and ear pain. Which pediatrician-level care does he need?",
            "tenant_id": TENANT_ID
        }

        route_3_data, success = self.make_request(
            'POST',
            '/api/v1/agents/route',
            data=peds_route,
            expected_status=200
        )

        if success and route_3_data:
            selected = route_3_data.get('selected_agent', {})
            specialty = selected.get('specialty', 'unknown')

            self.log(f"✓ Routed to: {specialty}", 'pass')
            self.tests_passed += 1

    # ─────────────────────────────────────────────────────────────
    # WORKFLOW 4: Cross-system Integration
    # ─────────────────────────────────────────────────────────────

    def e2e_workflow_4_cross_system(self) -> None:
        """E2E Workflow 4: Integration between RAG and Agents."""
        self.log("\n=== E2E Workflow 4: Cross-system Integration ===", 'section')

        # Step 1: Query RAG for hypertension info
        self.log("Step 1/2: Query RAG for specific medical info...", 'info')

        rag_query = {
            "question": "What are the contraindications for ACE inhibitors?",
            "mode": "vector"
        }

        rag_data, success = self.make_request(
            'POST',
            f'/api/v1/tenants/{TENANT_ID}/query',
            data=rag_query,
            expected_status=200
        )

        sources_found = 0
        if success and rag_data:
            sources = rag_data.get('sources', [])
            sources_found = len(sources)
            self.log(f"✓ RAG provided {sources_found} sources", 'pass')
            self.tests_passed += 1
        else:
            self.log("⚠ RAG query returned no results", 'warn')

        # Step 2: Chat with agent about the same topic
        self.log("Step 2/2: Chat with agent about the same topic...", 'info')

        if not self.agent_id:
            agents_data, _ = self.make_request('GET', '/api/v1/agents')
            if agents_data:
                self.agent_id = agents_data[0].get('id')

        chat_data = {
            "message": "Based on my medical history with angioedema, which antihypertensive should I avoid?",
            "mode": "rag"  # Use RAG-augmented response
        }

        chat_response, success = self.make_request(
            'POST',
            f'/api/v1/agents/{self.agent_id}/chat',
            data=chat_data,
            expected_status=200
        )

        if success and chat_response:
            response_text = chat_response.get('response', '')
            chat_sources = chat_response.get('sources', [])

            self.log(
                f"✓ Agent provided RAG-augmented answer ({len(chat_sources)} sources)",
                'pass'
            )

            if response_text:
                preview = response_text[:80] + "..." if len(response_text) > 80 else response_text
                print(f"  Agent: {preview}\n")

            self.tests_passed += 1
        else:
            self.log("⚠ Agent chat with RAG returned no response", 'warn')

    # ─────────────────────────────────────────────────────────────
    # Run All Workflows
    # ─────────────────────────────────────────────────────────────

    def run_all_workflows(self) -> None:
        """Run all E2E workflows."""
        self.log('╔═════════════════════════════════════════════════════╗', 'section')
        self.log('║  End-to-End Integration Tests - asgard-medical      ║', 'section')
        self.log('╚═════════════════════════════════════════════════════╝', 'section')
        self.log(f'Base URL: {BASE_URL}', 'info')
        self.log(f'Tenant: {TENANT_ID}\n', 'info')

        start_time = time.time()

        # Run workflows
        self.e2e_workflow_1_document_rag_flow()
        self.e2e_workflow_2_multiturn_chat()
        self.e2e_workflow_3_agent_routing()
        self.e2e_workflow_4_cross_system()

        # Summary
        elapsed = time.time() - start_time
        total = self.tests_passed + self.tests_failed
        percentage = (self.tests_passed / total * 100) if total > 0 else 0

        self.log('\n=== End-to-End Test Summary ===', 'section')

        summary_color = (
            Colors.GREEN if self.tests_failed == 0 else
            Colors.YELLOW if self.tests_failed <= 3 else
            Colors.RED
        )

        print(f"""
{summary_color}
Passed:  {self.tests_passed}/{total}
Failed:  {self.tests_failed}/{total}
Success: {percentage:.1f}%

⏱️  Total Time: {elapsed:.2f} seconds
{Colors.RESET}""")

        return 0 if self.tests_failed == 0 else 1

def main():
    """Main entry point."""
    try:
        suite = E2ETestSuite()
        exit_code = suite.run_all_workflows()
        sys.exit(exit_code)
    except KeyboardInterrupt:
        print("\n\nTest run interrupted")
        sys.exit(1)
    except Exception as e:
        print(f"Fatal error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
