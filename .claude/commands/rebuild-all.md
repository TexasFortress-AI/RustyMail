Rebuild and restart all RustyMail services

Perform a complete rebuild and restart of the RustyMail backend, frontend, and MCP stdio adapter using PM2 process manager.

## Environment Variables

**IMPORTANT**: RustyMail requires proper environment variable configuration:
- Backend reads from `/Users/au/src/RustyMail/.env`
- Frontend uses `dotenv-cli` to load the same `.env` file via package.json scripts
- Required vars: `REST_PORT`, `SSE_PORT`, `DASHBOARD_PORT`, `RUSTYMAIL_API_KEY`
- The application will **fail fast** if required env vars are missing

## Process Management

RustyMail uses **PM2** for reliable process management:
- Auto-restart on crashes
- Persistent process monitoring
- Centralized logging in `logs/` directory
- Process status: `pm2 status`
- View logs: `pm2 logs` or `pm2 logs rustymail-backend`

## Steps

1. **Stop all running services**:
   ```bash
   pm2 delete all 2>/dev/null || true
   ```
   Note: The `|| true` prevents errors if no processes are managed by PM2.

2. **Rebuild all components**:
   ```bash
   cargo build --release --bin rustymail-server
   cargo build --release --bin rustymail-mcp-stdio
   cd frontend/rustymail-app-main && npm run build && cd ../..
   ```

3. **Run tests** (optional, currently failing):
   ```bash
   cargo test --workspace  # Note: Limit output with timeout and head
   ```

4. **Start services with PM2**:
   ```bash
   pm2 start ecosystem.config.js
   pm2 save
   ```
   **Note**: The `ecosystem.config.js` file configures both backend and frontend. PM2 will:
   - Start backend (rustymail-server) which loads `.env` via dotenvy
   - Start frontend (npm run dev) which loads `../../.env` via dotenv-cli
   - Auto-restart services if they crash
   - Store logs in `logs/backend-*.log` and `logs/frontend-*.log`

5. **Verify services**:
   ```bash
   pm2 status                             # Should show both services running
   curl http://localhost:9437/api/health  # Backend health check
   curl http://localhost:9439             # Frontend serving
   ```

## PM2 Commands

```bash
# View service status
pm2 status

# View logs (all services)
pm2 logs

# View logs (specific service)
pm2 logs rustymail-backend
pm2 logs rustymail-frontend

# Restart services
pm2 restart rustymail-backend
pm2 restart rustymail-frontend
pm2 restart all

# Stop services (without deleting from PM2)
pm2 stop all

# Delete services from PM2 (stop + remove)
pm2 delete all

# Monitor services (live dashboard)
pm2 monit

# Save current process list (survives reboots)
pm2 save

# Resurrect saved processes after reboot
pm2 resurrect
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
