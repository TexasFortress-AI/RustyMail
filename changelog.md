# Changelog

## [Unreleased]

### Added

- **Testing Plan:** Expanded `mcp_testing_plan.md` to define a comprehensive E2E testing strategy.
  - Introduced Ports and Adapters pattern for IMAP backend testing (`MockImapAdapter`, `GoDaddyImapAdapter`, etc.).
  - Defined an "Interactive Diagnostic Console Port" with adapters for Stdio (`StdioConsoleAdapter`) and an SSE Web Dashboard (`SseDashboardConsoleAdapter`).
  - Included plans for an SSE Diagnostic Web Dashboard UI with statistics and console interaction.
  - Refined test suite structure (`mcp_stdio_api`, `mcp_sse_api`, `diagnostic_console`, `dashboard_ui`).
  - Added detailed Gherkin scenario examples for API and console testing.
  - Updated implementation checklist to reflect the new structure and features.
- **Dashboard Enhancements (Plan):** Further refined `mcp_testing_plan.md` for the SSE Diagnostic Dashboard:
  - Specified a required frontend technology stack (Next.js, Shadcn, tRPC, React Query, etc.).
  - Added requirement for a UI element to display the active IMAP backend adapter, with selection persistence in the browser.
  - Included specific Gherkin scenarios for testing adapter display and persistence. 