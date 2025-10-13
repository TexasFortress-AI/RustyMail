Rebuild and restart all RustyMail services

Perform a complete rebuild and restart of the RustyMail backend, frontend, and MCP stdio adapter.

## Environment Variables

**IMPORTANT**: RustyMail requires proper environment variable configuration:
- Backend reads from `/Users/au/src/RustyMail/.env`
- Frontend uses `dotenv-cli` to load the same `.env` file via package.json scripts
- Required vars: `REST_PORT`, `SSE_PORT`, `DASHBOARD_PORT`, `RUSTYMAIL_API_KEY`
- The application will **fail fast** if required env vars are missing

## Steps

1. **Kill all running processes** by port (reads from .env):
   ```bash
   lsof -ti:9437 | xargs kill -9 2>/dev/null || true  # Backend
   lsof -ti:9438 | xargs kill -9 2>/dev/null || true  # SSE server
   lsof -ti:9439 | xargs kill -9 2>/dev/null || true  # Frontend
   ```
   Note: Port-based killing ensures we only kill RustyMail processes. The `|| true` prevents errors if no process is on that port.

2. **Rebuild all components**:
   ```bash
   cargo build --release --bin rustymail-server
   cargo build --release --bin rustymail-mcp-stdio
   cd frontend/rustymail-app-main && npm run build
   ```

3. **Run tests** (optional, currently failing):
   ```bash
   cargo test --workspace  # Note: Limit output with timeout and head
   ```

4. **Start services** (in background):
   ```bash
   # Backend (loads .env automatically via dotenvy)
   ./target/release/rustymail-server &

   # Frontend (uses dotenv-cli configured in package.json)
   cd frontend/rustymail-app-main && npm run dev &
   ```
   **Note**: Frontend npm scripts automatically load `../../.env` via `dotenv -e` prefix (see package.json).

5. **Verify services**:
   ```bash
   lsof -i :9437 -i :9439 | grep LISTEN  # Should show both services
   curl http://localhost:9437/api/health  # Backend health check
   curl http://localhost:9439             # Frontend serving
   ```

## Architecture Notes

### Backend (Rust)
- Uses `dotenvy` crate to load `.env` at runtime
- Environment variables loaded in `src/config.rs`
- No hardcoded fallbacks - missing env vars cause panic with clear error messages
- Secrets stay server-side only

### Frontend (TypeScript/Vite)
- Uses `dotenv-cli` package to inject env vars before Vite starts
- `vite.config.ts` has `envDir: '../../'` to reference parent `.env`
- Validates required env vars at config load time
- No secrets exposed to client - only public config (ports, API URLs)
- Package.json scripts: `dotenv -e ../../.env -- vite`

### Why Not Copy .env?
- ❌ **Don't copy** `.env` to frontend directory (creates maintenance burden)
- ✅ **Do use** single source of truth (`/Users/au/src/RustyMail/.env`)
- ✅ **Do use** `dotenv-cli` to load parent `.env` at runtime
- ✅ **Do use** `envDir` in vite.config.ts for Vite's env file resolution

## Troubleshooting

- **Frontend fails to start**: Ensure `dotenv-cli` is installed (`npm install --save-dev dotenv-cli`)
- **"Environment variable required" errors**: Check `.env` file exists and has correct values
- **Wrong ports**: Verify `.env` has `REST_PORT=9437`, `DASHBOARD_PORT=9439`, etc.
