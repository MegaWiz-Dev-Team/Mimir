#!/usr/bin/env python3
"""
Agent Studio API Test Suite for asgard-medical Tenant

Tests agent management and chat functionality:
- List all agents
- Get agent templates
- Retrieve specific agent config
- Chat with an agent
- Conversation management
- Agent routing

Usage:
    python test_agents_api_medical.py
    MIMIR_URL=http://localhost:3002 python test_agents_api_medical.py
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
    MAGENTA = '\033[35m'

class AgentTestSuite:
    def __init__(self):
        self.tests_passed = 0
        self.tests_failed = 0
        self.session = requests.Session()
        self.agents = []

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

    def test_list_agents(self) -> None:
        """Test: List all available agents for the tenant."""
        path = f'/api/v1/agents'
        data, success = self.make_request('GET', path, expected_status=200)

        if success and isinstance(data, list):
            self.agents = data
            agent_count = len(data)

            self.log(f"✓ Found {agent_count} agents", 'pass')

            # Display agent details
            print(f"\n  {'Agent ID':<25} {'Name':<30} {'Specialty':<20} {'Model':<30}")
            print(f"  {'-'*25} {'-'*30} {'-'*20} {'-'*30}")

            for agent in data:
                agent_id = agent.get('id', 'N/A')[:25]
                name = agent.get('name', 'Unnamed')[:30]
                specialty = agent.get('specialty', 'General')[:20]
                model = agent.get('model_id', 'N/A')[:30]

                print(f"  {agent_id:<25} {name:<30} {specialty:<20} {model:<30}")

            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to list agents", 'fail')
            self.tests_failed += 1

    def test_get_agent_templates(self) -> None:
        """Test: Get available agent templates."""
        path = f'/api/v1/agents/templates'
        data, success = self.make_request('GET', path, expected_status=200)

        if success and isinstance(data, list):
            template_count = len(data)
            self.log(f"✓ Found {template_count} agent templates", 'pass')

            # Display first few templates
            for template in data[:3]:
                template_name = template.get('name', 'Unknown')
                template_desc = template.get('description', '')[:60]
                print(f"  - {template_name}: {template_desc}...")

            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to get templates", 'fail')
            self.tests_failed += 1

    def test_get_specific_agent(self) -> None:
        """Test: Get details of a specific agent."""
        if not self.agents:
            self.log("⊘ Skipping: No agents available", 'warn')
            return

        # Test first agent
        agent_id = self.agents[0].get('id')
        path = f'/api/v1/agents/{agent_id}'
        data, success = self.make_request('GET', path, expected_status=200)

        if success and data:
            name = data.get('name', 'Unknown')
            specialty = data.get('specialty', 'General')
            use_rag = data.get('use_rag', False)
            temperature = data.get('temperature', 0)

            self.log(
                f"✓ Retrieved agent '{name}' (RAG: {use_rag}, Temp: {temperature})",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to get agent details", 'fail')
            self.tests_failed += 1

    def test_agent_chat(self) -> None:
        """Test: Send a chat message to an agent."""
        if not self.agents:
            self.log("⊘ Skipping: No agents available", 'warn')
            return

        # Use first agent for chat
        agent_id = self.agents[0].get('id')
        path = f'/api/v1/agents/{agent_id}/chat'

        chat_request = {
            "message": "What are common symptoms of respiratory infections?",
            "model": "auto"  # Use agent's default model
        }

        data, success = self.make_request('POST', path, data=chat_request, expected_status=200)

        if success and data:
            response = data.get('response', '')
            conversation_id = data.get('conversation_id', 'N/A')

            if response:
                preview = response[:100] + "..." if len(response) > 100 else response
                self.log(
                    f"✓ Chat successful (Conv: {conversation_id})",
                    'pass'
                )
                print(f"\n  Response Preview:\n  \"{preview}\"\n")
                self.tests_passed += 1
            else:
                self.log(f"✗ No response from agent", 'fail')
                self.tests_failed += 1
        else:
            self.log(f"✗ Chat request failed", 'fail')
            self.tests_failed += 1

    def test_agent_conversations(self) -> None:
        """Test: List conversations for an agent."""
        if not self.agents:
            self.log("⊘ Skipping: No agents available", 'warn')
            return

        agent_id = self.agents[0].get('id')
        path = f'/api/v1/agents/{agent_id}/conversations'

        data, success = self.make_request('GET', path, expected_status=200)

        if success and isinstance(data, list):
            conv_count = len(data)
            self.log(
                f"✓ Retrieved {conv_count} conversations for agent",
                'pass'
            )

            # Show first few conversations
            for conv in data[:3]:
                conv_id = conv.get('id', 'N/A')
                created = conv.get('created_at', 'N/A')
                print(f"  - Conversation {conv_id} (Created: {created})")

            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to list conversations", 'fail')
            self.tests_failed += 1

    def test_agent_routing(self) -> None:
        """Test: Route a question to appropriate specialist agent."""
        path = f'/api/v1/agents/route'

        route_request = {
            "question": "I have chest pain and difficulty breathing. What specialist should I see?",
            "tenant_id": TENANT_ID
        }

        data, success = self.make_request('POST', path, data=route_request, expected_status=200)

        if success and data:
            selected_agent = data.get('selected_agent', {})
            agent_name = selected_agent.get('name', 'Unknown')
            specialty = selected_agent.get('specialty', 'Unknown')
            confidence = data.get('confidence', 0)

            self.log(
                f"✓ Routed to specialist: '{agent_name}' ({specialty}) - Confidence: {confidence:.2f}",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Agent routing failed", 'fail')
            self.tests_failed += 1

    def test_agent_list_query(self) -> None:
        """Test: List agents with query filters."""
        path = f'/api/v1/agents?specialty=cardiology&limit=5'

        data, success = self.make_request('GET', path, expected_status=200)

        if success and isinstance(data, list):
            self.log(
                f"✓ Filtered agents by specialty: {len(data)} results",
                'pass'
            )
            self.tests_passed += 1
        else:
            self.log(f"✗ Failed to filter agents", 'fail')
            self.tests_failed += 1

    def run_all_tests(self) -> None:
        """Run complete test suite."""
        self.log('=== Agent Studio API Test Suite ===', 'section')
        self.log(f'Base URL: {BASE_URL}', 'info')
        self.log(f'Tenant: {TENANT_ID}\n', 'info')

        # Phase 1: Agent Discovery
        self.log('Phase 1: Agent Discovery', 'section')
        self.test_list_agents()
        self.test_get_agent_templates()

        # Phase 2: Agent Management
        self.log('\nPhase 2: Agent Management', 'section')
        self.test_get_specific_agent()

        # Phase 3: Agent Chat
        self.log('\nPhase 3: Agent Chat & Conversations', 'section')
        self.test_agent_chat()
        self.test_agent_conversations()

        # Phase 4: Advanced Features
        self.log('\nPhase 4: Advanced Features', 'section')
        self.test_agent_routing()
        self.test_agent_list_query()

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
        suite = AgentTestSuite()
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
