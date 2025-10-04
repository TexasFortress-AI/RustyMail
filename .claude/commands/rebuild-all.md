Rebuild and restart all RustyMail services

Perform a complete rebuild and restart of the RustyMail backend, frontend, and MCP stdio adapter.

Steps:

1. Kill all running processes:
   - Kill backend server (rustymail-server)
   - Kill frontend dev server (vite on port 5173 or 9439)

2. Rebuild all components:
   - Build backend: `cargo build --release --bin rustymail-server`
   - Build frontend: `cd frontend/rustymail-app-main && npm run build`
   - Build MCP stdio: `cargo build --release --bin rustymail-mcp-stdio`

3. Run tests:
   - Run Rust tests: `cargo test --workspace` (note: some tests may fail due to outdated test code, production code is fine)

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
- Unit tests may have compilation errors but production code compiles fine
