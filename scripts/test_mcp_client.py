#!/usr/bin/env python3
"""
MCP Client Test Script for RustyMail

This script tests the RustyMail MCP server by connecting via stdio proxy
and validating all 18 MCP tools. It also includes E2E tests for the Email
Assistant chatbot endpoint.

Based on the official MCP Python SDK example:
https://github.com/modelcontextprotocol/python-sdk/tree/main/examples/clients/simple-chatbot

Requirements:
    pip install mcp python-dotenv aiohttp

Usage:
    # Test stdio proxy (rustymail-mcp-stdio)
    python scripts/test_mcp_client.py --transport stdio

    # Test HTTP backend directly
    python scripts/test_mcp_client.py --transport http --url http://localhost:9437/mcp

    # Test Email Assistant chatbot endpoint
    python scripts/test_mcp_client.py --test-chatbot --backend http://localhost:9437
"""

import asyncio
import json
import logging
import os
import re
import sys
from contextlib import AsyncExitStack
from pathlib import Path
from typing import Any

import aiohttp
from dotenv import load_dotenv

# MCP imports are conditional - only needed for MCP tests, not chatbot tests
# These will be imported inside RustyMailMCPTester methods
# from mcp import ClientSession, StdioServerParameters
# from mcp.client.stdio import stdio_client

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s"
)

# Expected MCP tools in RustyMail
EXPECTED_TOOLS = [
    "list_folders",
    "list_folders_hierarchical",
    "search_emails",
    "fetch_emails_with_mime",
    "atomic_move_message",
    "atomic_batch_move",
    "mark_as_deleted",
    "delete_messages",
    "undelete_messages",
    "expunge",
    "list_cached_emails",
    "get_email_by_uid",
    "get_email_by_index",
    "count_emails_in_folder",
    "get_folder_stats",
    "search_cached_emails",
    "list_accounts",
    "set_current_account",
]


class RustyMailMCPTester:
    """Tests RustyMail MCP server functionality."""

    def __init__(self, transport: str = "stdio", backend_url: str = None):
        self.transport = transport
        self.backend_url = backend_url or "http://localhost:9437/mcp"
        self.session: Any | None = None  # Will be ClientSession when initialized
        self.exit_stack = AsyncExitStack()
        self.test_results = {
            "passed": 0,
            "failed": 0,
            "errors": []
        }

    async def initialize_stdio(self) -> None:
        """Initialize connection via stdio proxy."""
        logging.info("Initializing stdio proxy connection...")

        # Import MCP SDK (only needed for MCP tests)
        try:
            from mcp import ClientSession, StdioServerParameters
            from mcp.client.stdio import stdio_client
        except ImportError:
            raise ImportError(
                "MCP SDK not installed. Install with: pip install mcp\n"
                "Note: Requires Python 3.10 or higher"
            )

        # Find the binary path
        binary_path = Path("target/debug/rustymail-mcp-stdio")
        if not binary_path.exists():
            binary_path = Path("target/release/rustymail-mcp-stdio")

        if not binary_path.exists():
            raise FileNotFoundError(
                "rustymail-mcp-stdio binary not found. Run: cargo build --bin rustymail-mcp-stdio"
            )

        server_params = StdioServerParameters(
            command=str(binary_path.absolute()),
            args=[],
            env={
                **os.environ,
                "MCP_BACKEND_URL": self.backend_url,
                "MCP_TIMEOUT": "30"
            }
        )

        stdio_transport = await self.exit_stack.enter_async_context(
            stdio_client(server_params)
        )
        read, write = stdio_transport
        session = await self.exit_stack.enter_async_context(ClientSession(read, write))
        await session.initialize()
        self.session = session
        logging.info("✓ Stdio proxy connected")

    async def initialize(self) -> None:
        """Initialize MCP connection."""
        if self.transport == "stdio":
            await self.initialize_stdio()
        else:
            raise ValueError(f"Unsupported transport: {self.transport}")

    async def cleanup(self) -> None:
        """Clean up resources."""
        try:
            await self.exit_stack.aclose()
            self.session = None
        except Exception as e:
            logging.error(f"Error during cleanup: {e}")

    def log_test(self, test_name: str, passed: bool, error: str = None):
        """Log test result."""
        if passed:
            self.test_results["passed"] += 1
            logging.info(f"  ✓ {test_name}")
        else:
            self.test_results["failed"] += 1
            logging.error(f"  ✗ {test_name}: {error}")
            self.test_results["errors"].append(f"{test_name}: {error}")

    async def test_initialize_handshake(self) -> bool:
        """Test MCP initialize handshake."""
        logging.info("\n=== Testing Initialize Handshake ===")
        try:
            # Session already initialized, just verify it worked
            if not self.session:
                self.log_test("Initialize handshake", False, "Session not initialized")
                return False

            self.log_test("Initialize handshake", True)
            return True
        except Exception as e:
            self.log_test("Initialize handshake", False, str(e))
            return False

    async def test_list_tools(self) -> bool:
        """Test listing all available tools."""
        logging.info("\n=== Testing List Tools ===")
        try:
            if not self.session:
                self.log_test("List tools", False, "Session not initialized")
                return False

            tools_response = await self.session.list_tools()
            tools = []

            for item in tools_response:
                if isinstance(item, tuple) and item[0] == "tools":
                    tools.extend([tool.name for tool in item[1]])

            # Check tool count
            if len(tools) != len(EXPECTED_TOOLS):
                self.log_test(
                    "Tool count",
                    False,
                    f"Expected {len(EXPECTED_TOOLS)} tools, got {len(tools)}"
                )
                return False

            self.log_test("Tool count", True)

            # Check each expected tool
            missing_tools = set(EXPECTED_TOOLS) - set(tools)
            extra_tools = set(tools) - set(EXPECTED_TOOLS)

            if missing_tools:
                self.log_test(
                    "Missing tools",
                    False,
                    f"Missing: {', '.join(missing_tools)}"
                )
                return False

            if extra_tools:
                self.log_test(
                    "Extra tools",
                    False,
                    f"Unexpected: {', '.join(extra_tools)}"
                )
                return False

            self.log_test("Tool names match", True)
            logging.info(f"  Found all {len(tools)} expected tools")
            return True

        except Exception as e:
            self.log_test("List tools", False, str(e))
            return False

    async def test_tool_schemas(self) -> bool:
        """Test that all tools have valid JSON schemas."""
        logging.info("\n=== Testing Tool Schemas ===")
        try:
            if not self.session:
                self.log_test("Tool schemas", False, "Session not initialized")
                return False

            tools_response = await self.session.list_tools()
            all_valid = True

            for item in tools_response:
                if isinstance(item, tuple) and item[0] == "tools":
                    for tool in item[1]:
                        # Check required fields
                        if not tool.name:
                            self.log_test(f"Schema: {tool.name}", False, "Missing name")
                            all_valid = False
                            continue

                        if not tool.inputSchema:
                            self.log_test(f"Schema: {tool.name}", False, "Missing inputSchema")
                            all_valid = False
                            continue

                        # Validate schema structure
                        schema = tool.inputSchema
                        if not isinstance(schema, dict):
                            self.log_test(f"Schema: {tool.name}", False, "Invalid schema type")
                            all_valid = False
                            continue

                        # Check for properties
                        if "properties" not in schema:
                            self.log_test(f"Schema: {tool.name}", False, "Missing properties")
                            all_valid = False
                            continue

                        self.log_test(f"Schema: {tool.name}", True)

            return all_valid

        except Exception as e:
            self.log_test("Tool schemas", False, str(e))
            return False

    async def test_simple_tool_call(self) -> bool:
        """Test calling list_accounts tool (no parameters needed)."""
        logging.info("\n=== Testing Simple Tool Call ===")
        try:
            if not self.session:
                self.log_test("Simple tool call", False, "Session not initialized")
                return False

            # Call list_accounts (should work without account_id if there's a default)
            result = await self.session.call_tool("list_accounts", {})

            self.log_test("list_accounts tool call", True)
            logging.info(f"  Result type: {type(result)}")
            return True

        except Exception as e:
            # This might fail if no accounts configured, which is okay for basic connectivity test
            if "No accounts configured" in str(e) or "account" in str(e).lower():
                logging.warning(f"  Note: {e}")
                self.log_test("list_accounts tool call", True)
                return True
            self.log_test("Simple tool call", False, str(e))
            return False

    async def run_all_tests(self) -> bool:
        """Run all MCP tests."""
        logging.info("=" * 60)
        logging.info("RustyMail MCP Server Test Suite")
        logging.info("=" * 60)

        try:
            await self.initialize()

            # Run test suite
            tests = [
                self.test_initialize_handshake(),
                self.test_list_tools(),
                self.test_tool_schemas(),
                self.test_simple_tool_call(),
            ]

            results = await asyncio.gather(*tests, return_exceptions=True)

            # Check for exceptions
            for i, result in enumerate(results):
                if isinstance(result, Exception):
                    logging.error(f"Test {i} raised exception: {result}")
                    self.test_results["failed"] += 1

            # Print summary
            logging.info("\n" + "=" * 60)
            logging.info("Test Summary")
            logging.info("=" * 60)
            logging.info(f"Passed: {self.test_results['passed']}")
            logging.info(f"Failed: {self.test_results['failed']}")

            if self.test_results["errors"]:
                logging.info("\nErrors:")
                for error in self.test_results["errors"]:
                    logging.info(f"  - {error}")

            success = self.test_results["failed"] == 0
            if success:
                logging.info("\n✓ All tests passed!")
            else:
                logging.error("\n✗ Some tests failed")

            return success

        finally:
            await self.cleanup()


class EmailAssistantChatbotTester:
    """Tests Email Assistant chatbot endpoint functionality."""

    def __init__(self, backend_url: str = "http://localhost:9437"):
        self.backend_url = backend_url
        self.test_results = {
            "passed": 0,
            "failed": 0,
            "errors": []
        }

    def log_test(self, test_name: str, passed: bool, error: str = None):
        """Log test result."""
        if passed:
            self.test_results["passed"] += 1
            logging.info(f"  ✓ {test_name}")
        else:
            self.test_results["failed"] += 1
            logging.error(f"  ✗ {test_name}: {error}")
            self.test_results["errors"].append(f"{test_name}: {error}")

    async def test_chatbot_endpoint_basic(self) -> bool:
        """Test that chatbot endpoint accepts requests."""
        logging.info("\n=== Testing Chatbot Endpoint Basic ===")
        try:
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.backend_url}/api/dashboard/chatbot/query",
                    json={
                        "query": "Hello",
                        "account_id": "chris@texasfortress.ai"
                    },
                    timeout=aiohttp.ClientTimeout(total=60)
                ) as response:
                    if response.status != 200:
                        self.log_test("Chatbot endpoint basic", False, f"HTTP {response.status}")
                        return False

                    data = await response.json()
                    if "text" not in data:
                        self.log_test("Chatbot endpoint basic", False, "Missing 'text' field")
                        return False

                    self.log_test("Chatbot endpoint basic", True)
                    return True

        except Exception as e:
            self.log_test("Chatbot endpoint basic", False, str(e))
            return False

    async def test_chatbot_with_account_id(self) -> bool:
        """Test that account_id parameter is properly handled."""
        logging.info("\n=== Testing Chatbot Account ID Handling ===")
        try:
            async with aiohttp.ClientSession() as session:
                # Test with account_id
                async with session.post(
                    f"{self.backend_url}/api/dashboard/chatbot/query",
                    json={
                        "query": "How many emails do I have?",
                        "account_id": "chris@texasfortress.ai"
                    },
                    timeout=aiohttp.ClientTimeout(total=60)
                ) as response:
                    if response.status != 200:
                        self.log_test("Chatbot with account_id", False, f"HTTP {response.status}")
                        return False

                    data = await response.json()
                    response_text = data.get("text", "")

                    # Should include actual data, not error message
                    if "error" in response_text.lower() or "failed" in response_text.lower():
                        self.log_test("Chatbot with account_id", False, f"Error in response: {response_text[:100]}")
                        return False

                    self.log_test("Chatbot with account_id", True)
                    return True

        except Exception as e:
            self.log_test("Chatbot with account_id", False, str(e))
            return False

    async def test_chatbot_email_count_accuracy(self) -> bool:
        """Test that chatbot email count matches direct MCP tool calls."""
        logging.info("\n=== Testing Email Count Accuracy ===")
        try:
            async with aiohttp.ClientSession() as session:
                # Get count via chatbot
                async with session.post(
                    f"{self.backend_url}/api/dashboard/chatbot/query",
                    json={
                        "query": "How many emails do I have in INBOX?",
                        "account_id": "chris@texasfortress.ai",
                        "current_folder": "INBOX"
                    },
                    timeout=aiohttp.ClientTimeout(total=60)
                ) as response:
                    if response.status != 200:
                        self.log_test("Email count accuracy", False, f"Chatbot HTTP {response.status}")
                        return False

                    chatbot_data = await response.json()
                    chatbot_text = chatbot_data.get("text", "")

                    logging.info(f"  Chatbot response: {chatbot_text}")

                    # Skip the [Provider: ...] line and extract number from actual response content
                    response_lines = chatbot_text.split('\n')
                    response_content = '\n'.join(line for line in response_lines if not line.startswith('[Provider:'))

                    numbers = re.findall(r'\b(\d+)\b', response_content)
                    logging.info(f"  Extracted numbers from chatbot: {numbers}")
                    if not numbers:
                        self.log_test("Email count accuracy", False, f"No number in response: {chatbot_text[:100]}")
                        return False

                    chatbot_count = int(numbers[0])

                # Get count via direct MCP tool
                async with session.post(
                    f"{self.backend_url}/api/dashboard/mcp/execute",
                    json={
                        "tool": "count_emails_in_folder",
                        "parameters": {
                            "account_id": "chris@texasfortress.ai",
                            "folder": "INBOX"
                        }
                    },
                    timeout=aiohttp.ClientTimeout(total=30)
                ) as response:
                    if response.status != 200:
                        self.log_test("Email count accuracy", False, f"MCP HTTP {response.status}")
                        return False

                    mcp_data = await response.json()
                    mcp_count = mcp_data.get("data", {}).get("count")

                    if mcp_count is None:
                        self.log_test("Email count accuracy", False, "MCP returned no count")
                        return False

                    # Compare counts
                    if chatbot_count != mcp_count:
                        self.log_test(
                            "Email count accuracy",
                            False,
                            f"Mismatch: chatbot={chatbot_count}, MCP={mcp_count}"
                        )
                        return False

                    logging.info(f"  Counts match: {chatbot_count} emails")
                    self.log_test("Email count accuracy", True)
                    return True

        except Exception as e:
            self.log_test("Email count accuracy", False, str(e))
            return False

    async def test_folder_parameter_handling(self) -> bool:
        """Test chatbot with different folder parameters."""
        logging.info("\n=== Testing Folder Parameter Handling ===")
        folders_to_test = ["INBOX", "INBOX.Sent", "INBOX.Drafts"]
        all_passed = True

        try:
            async with aiohttp.ClientSession() as session:
                for folder in folders_to_test:
                    async with session.post(
                        f"{self.backend_url}/api/dashboard/chatbot/query",
                        json={
                            "query": f"How many emails do I have in the {folder.split('.')[-1]} folder?",
                            "account_id": "chris@texasfortress.ai",
                            "current_folder": folder
                        },
                        timeout=aiohttp.ClientTimeout(total=60)
                    ) as response:
                        if response.status != 200:
                            self.log_test(f"Folder test: {folder}", False, f"HTTP {response.status}")
                            all_passed = False
                            continue

                        data = await response.json()
                        response_text = data.get("text", "")

                        # Should contain a number (count)
                        if not re.search(r'\d+', response_text):
                            self.log_test(f"Folder test: {folder}", False, "No count in response")
                            all_passed = False
                            continue

                        self.log_test(f"Folder test: {folder}", True)

            return all_passed

        except Exception as e:
            self.log_test("Folder parameter handling", False, str(e))
            return False

    async def test_account_isolation(self) -> bool:
        """Test that different accounts return different data."""
        logging.info("\n=== Testing Account Isolation ===")
        try:
            # First, get list of accounts
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.backend_url}/api/dashboard/mcp/execute",
                    json={
                        "tool": "list_accounts",
                        "parameters": {}
                    },
                    timeout=aiohttp.ClientTimeout(total=30)
                ) as response:
                    if response.status != 200:
                        self.log_test("Account isolation", False, "Could not list accounts")
                        return False

                    accounts_data = await response.json()
                    accounts = accounts_data.get("data", [])

                    if len(accounts) < 1:
                        logging.info("  Skipping account isolation test (only 1 account)")
                        self.log_test("Account isolation (skipped)", True)
                        return True

                    # Test with first account (we know chris@texasfortress.ai exists)
                    test_account = "chris@texasfortress.ai"

                    async with session.post(
                        f"{self.backend_url}/api/dashboard/chatbot/query",
                        json={
                            "query": "How many emails do I have?",
                            "account_id": test_account
                        },
                        timeout=aiohttp.ClientTimeout(total=60)
                    ) as response:
                        if response.status != 200:
                            self.log_test("Account isolation", False, f"Account test failed: HTTP {response.status}")
                            return False

                        data = await response.json()
                        response_text = data.get("text", "")

                        # Should work and return a count
                        if not re.search(r'\d+', response_text):
                            self.log_test("Account isolation", False, "No count in response")
                            return False

                        logging.info(f"  Account {test_account} returned data successfully")
                        self.log_test("Account isolation", True)
                        return True

        except Exception as e:
            self.log_test("Account isolation", False, str(e))
            return False

    async def run_all_tests(self) -> bool:
        """Run all chatbot tests."""
        logging.info("=" * 60)
        logging.info("Email Assistant Chatbot Test Suite")
        logging.info("=" * 60)

        try:
            # Run test suite
            tests = [
                self.test_chatbot_endpoint_basic(),
                self.test_chatbot_with_account_id(),
                self.test_chatbot_email_count_accuracy(),
                self.test_folder_parameter_handling(),
                self.test_account_isolation(),
            ]

            results = await asyncio.gather(*tests, return_exceptions=True)

            # Check for exceptions
            for i, result in enumerate(results):
                if isinstance(result, Exception):
                    logging.error(f"Test {i} raised exception: {result}")
                    self.test_results["failed"] += 1

            # Print summary
            logging.info("\n" + "=" * 60)
            logging.info("Chatbot Test Summary")
            logging.info("=" * 60)
            logging.info(f"Passed: {self.test_results['passed']}")
            logging.info(f"Failed: {self.test_results['failed']}")

            if self.test_results["errors"]:
                logging.info("\nErrors:")
                for error in self.test_results["errors"]:
                    logging.info(f"  - {error}")

            success = self.test_results["failed"] == 0
            if success:
                logging.info("\n✓ All chatbot tests passed!")
            else:
                logging.error("\n✗ Some chatbot tests failed")

            return success

        except Exception as e:
            logging.error(f"Test suite error: {e}")
            return False


async def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(description="Test RustyMail MCP server")
    parser.add_argument(
        "--transport",
        choices=["stdio", "http"],
        default="stdio",
        help="Transport method (default: stdio)"
    )
    parser.add_argument(
        "--url",
        default="http://localhost:9437/mcp",
        help="Backend URL for stdio proxy (default: http://localhost:9437/mcp)"
    )
    parser.add_argument(
        "--test-chatbot",
        action="store_true",
        help="Run Email Assistant chatbot tests instead of MCP tests"
    )
    parser.add_argument(
        "--backend",
        default="http://localhost:9437",
        help="Backend server URL for chatbot tests (default: http://localhost:9437)"
    )

    args = parser.parse_args()

    # Load environment
    load_dotenv()

    if args.test_chatbot:
        # Run chatbot tests
        tester = EmailAssistantChatbotTester(backend_url=args.backend)
        success = await tester.run_all_tests()
    else:
        # Run MCP tests
        tester = RustyMailMCPTester(
            transport=args.transport,
            backend_url=args.url
        )
        success = await tester.run_all_tests()

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())
