Rebuild and restart all RustyMail services

Perform a complete rebuild and restart of the RustyMail backend, frontend, and MCP stdio adapter.

Steps:

1. Kill all running processes by port (from .env.example):
   - Kill backend on port 9437: `lsof -ti:9437 | xargs kill -9 2>/dev/null || true`
   - Kill SSE server on port 9438: `lsof -ti:9438 | xargs kill -9 2>/dev/null || true`
   - Kill frontend on port 9439: `lsof -ti:9439 | xargs kill -9 2>/dev/null || true`
   - Note: Using port-based killing ensures we only kill RustyMail processes, not other projects. The `|| true` prevents errors if no process is on that port.

2. Rebuild all components:
   - Build backend: `cargo build --release --bin rustymail-server`
   - Build frontend: `cd frontend/rustymail-app-main && npm run build`
   - Build MCP stdio: `cargo build --release --bin rustymail-mcp-stdio`

3. Run tests:
   - Run Rust tests: `cargo test --workspace` (note: limit output with timeout and head to avoid context overflow)

4. Restart services:
   - Start backend: `./target/release/rustymail-server` (run in background)
   - Start frontend: `cd frontend/rustymail-app-main && npm run dev` (run in background)

5. Verify services are running:
   - Backend should be on http://localhost:9437
   - Frontend should be on http://localhost:9439

Important notes:
- Always use full absolute paths when starting services in background
- Wait for builds to complete before starting services
- Check logs to ensure services started correctly
- If unit tests fail to compile, fix them.
- If unit tests prove that the software is broken, fix the software.
