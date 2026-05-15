#!/usr/bin/env python3
"""
RAG Studio Test Suite for asgard-medical Tenant

Tests the RAG Playground functionality on the medical tenant:
- Health checks
- Tenant verification
- Document ingestion
- Vector search
- Hybrid search
- Query responses

Usage:
    python test_rag_playground_medical.py
    MIMIR_URL=http://localhost:3002 python test_rag_playground_medical.py
"""

import os
import sys
import json
import requests
from datetime import datetime
from typing import Dict, Any, Optional
from urllib.parse import urljoin

# Configuration
BASE_URL = os.environ.get('MIMIR_URL', 'http://localhost:3002')
TENANT_ID = 'asgard-medical'
TEST_TIMEOUT = 30

# ANSI Color codes
class Colors:
    RESET = '\033[0m'
    GREEN = '\033[32m'
    RED = '\033[31m'
    YELLOW = '\033[33m'
    BLUE = '\033[34m'
    CYAN = '\033[36m'

class RAGTestSuite:
    def __init__(self):
        self.tests_passed = 0
        self.tests_failed = 0
        self.session = requests.Session()

    def log(self, message: str, level: str = 'info') -> None:
        """Log message with timestamp and color."""
        timestamp = datetime.now().isoformat()

        color_map = {
            'pass': Colors.GREEN,
            'fail': Colors.RED,
            'warn': Colors.YELLOW,
            'section': Colors.CYAN,
            'info': Colors.BLUE
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
            elif method == 'DELETE':
                resp = self.session.delete(url, timeout=TEST_TIMEOUT)
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

    def test_health_check(self) -> None:
        """Test: Service health check."""
        data, success = self.make_request('GET', '/health', expected_status=200)

        if success and data and data.get('status') == 'ok':
            self.log('✓ Health check passed', 'pass')
            self.tests_passed += 1
        else:
            self.log(f"✗ Health check failed", 'fail')
            self.tests_failed += 1

    def test_tenant_exists(self) -> None:
        """Test: Verify asgard-medical tenant exists."""
        path = f'/api/v1/tenants/{TENANT_ID}'
        data, success = self.make_request('GET', path, expected_status=200)

        if success and data:
            domain = data.get('domain', 'unknown')
            self.log(
                f"✓ Tenant '{TENANT_ID}' exists (Domain: {domain})",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Tenant '{TENANT_ID}' not found", 'fail')
            self.tests_failed += 1

    def test_list_documents(self) -> None:
        """Test: List all ingested documents."""
        path = f'/api/v1/tenants/{TENANT_ID}/ingest/documents'
        data, success = self.make_request('GET', path, expected_status=200)

        if success and isinstance(data, list):
            self.log(
                f"✓ Documents listed: {len(data)} documents found",
                'pass'
            )
            # Show first 3 documents
            for doc in data[:3]:
                doc_title = doc.get('title', 'Untitled')
                doc_id = doc.get('id', 'N/A')
                print(f"  - \"{doc_title}\" (ID: {doc_id})")
            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to list documents", 'fail')
            self.tests_failed += 1

    def test_ingest_document(self) -> None:
        """Test: Ingest a test medical document."""
        test_doc = {
            "title": "RAG Playground Test Document",
            "content": """# Medical Knowledge Test Document

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
Based on standard medical protocols and evidence-based medicine.""",
            "source": "rag-playground-test"
        }

        path = f'/api/v1/tenants/{TENANT_ID}/ingest'
        data, success = self.make_request('POST', path, data=test_doc, expected_status=200)

        if success and data:
            doc_id = data.get('document_id', 'N/A')
            status = data.get('status', 'unknown')
            self.log(
                f"✓ Document ingested (ID: {doc_id}, Status: {status})",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Document ingestion failed", 'fail')
            self.tests_failed += 1

    def test_vector_search(self) -> None:
        """Test: Vector search query."""
        query = {
            "question": "What are the symptoms of respiratory infections?",
            "mode": "vector"
        }

        path = f'/api/v1/tenants/{TENANT_ID}/query'
        data, success = self.make_request('POST', path, data=query, expected_status=200)

        if success and data:
            mode_used = data.get('mode_used', 'unknown')
            sources_count = len(data.get('sources', []))
            answer = data.get('answer', '')

            self.log(
                f"✓ Vector search successful (Mode: {mode_used}, Sources: {sources_count})",
                'pass'
            )

            if answer:
                preview = answer[:120] + "..." if len(answer) > 120 else answer
                print(f"\n  Answer Preview:\n  \"{preview}\"\n")

            self.tests_passed += 1
        else:
            self.log(f"✗ Vector search failed", 'fail')
            self.tests_failed += 1

    def test_hybrid_search(self) -> None:
        """Test: Hybrid search query."""
        query = {
            "question": "What treatment options are available for respiratory infections?",
            "mode": "hybrid"
        }

        path = f'/api/v1/tenants/{TENANT_ID}/query'
        data, success = self.make_request('POST', path, data=query, expected_status=200)

        if success and data:
            mode_used = data.get('mode_used', 'unknown')
            sources_count = len(data.get('sources', []))

            self.log(
                f"✓ Hybrid search successful (Mode: {mode_used}, Sources: {sources_count})",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Hybrid search failed", 'fail')
            self.tests_failed += 1

    def test_general_query(self) -> None:
        """Test: General query without mode specification."""
        query = {
            "question": "General medical information about common diseases"
        }

        path = f'/api/v1/tenants/{TENANT_ID}/query'
        data, success = self.make_request('POST', path, data=query, expected_status=200)

        if success and data:
            sources_count = len(data.get('sources', []))

            self.log(
                f"✓ General query successful (Sources: {sources_count})",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ General query failed", 'fail')
            self.tests_failed += 1

    def run_all_tests(self) -> None:
        """Run complete test suite."""
        self.log('=== RAG Studio Test Suite for asgard-medical ===', 'section')
        self.log(f'Base URL: {BASE_URL}', 'info')
        self.log(f'Tenant: {TENANT_ID}\n', 'info')

        # Phase 1: Infrastructure
        self.log('Phase 1: Infrastructure & Connectivity', 'section')
        self.test_health_check()

        # Phase 2: Tenant Verification
        self.log('\nPhase 2: Tenant Verification', 'section')
        self.test_tenant_exists()

        # Phase 3: Document Management
        self.log('\nPhase 3: Document Management', 'section')
        self.test_list_documents()
        self.test_ingest_document()

        # Phase 4: RAG Queries
        self.log('\nPhase 4: RAG Queries', 'section')
        self.test_vector_search()
        self.test_hybrid_search()
        self.test_general_query()

        # Summary
        self.log('\n=== Test Summary ===', 'section')
        total = self.tests_passed + self.tests_failed
        percentage = (self.tests_passed / total * 100) if total > 0 else 0

        summary_color = (
            Colors.GREEN if self.tests_failed == 0 else
            Colors.YELLOW if self.tests_failed <= 2 else
            Colors.RED
        )

        print(f"""
{summary_color}
Passed:  {self.tests_passed}/{total}
Failed:  {self.tests_failed}/{total}
Success: {percentage:.1f}%
{Colors.RESET}""")

        return 0 if self.tests_failed == 0 else 1

def main():
    """Main entry point."""
    try:
        suite = RAGTestSuite()
        exit_code = suite.run_all_tests()
        sys.exit(exit_code)
    except KeyboardInterrupt:
        print("\n\nTest run interrupted")
        sys.exit(1)
    except Exception as e:
        print(f"Fatal error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
