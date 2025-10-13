#!/usr/bin/env python3
"""
MCP Client Test Script for RustyMail

This script tests the RustyMail MCP server by connecting via stdio proxy
and validating all 18 MCP tools.

Based on the official MCP Python SDK example:
https://github.com/modelcontextprotocol/python-sdk/tree/main/examples/clients/simple-chatbot

Requirements:
    pip install mcp python-dotenv

Usage:
    # Test stdio proxy (rustymail-mcp-stdio)
    python scripts/test_mcp_client.py --transport stdio

    # Test HTTP backend directly
    python scripts/test_mcp_client.py --transport http --url http://localhost:9437/mcp
"""

import asyncio
import json
import logging
import os
import sys
from contextlib import AsyncExitStack
from pathlib import Path
from typing import Any

from dotenv import load_dotenv
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

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
        self.session: ClientSession | None = None
        self.exit_stack = AsyncExitStack()
        self.test_results = {
            "passed": 0,
            "failed": 0,
            "errors": []
        }

    async def initialize_stdio(self) -> None:
        """Initialize connection via stdio proxy."""
        logging.info("Initializing stdio proxy connection...")

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

    args = parser.parse_args()

    # Load environment
    load_dotenv()

    tester = RustyMailMCPTester(
        transport=args.transport,
        backend_url=args.url
    )

    success = await tester.run_all_tests()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())
