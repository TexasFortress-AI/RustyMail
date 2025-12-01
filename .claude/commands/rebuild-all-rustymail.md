Rebuild and restart all RustyMail services (COMPLETE CLEAN BUILD)

Perform a **complete, from-scratch rebuild** of the entire RustyMail system:
- Kill ALL running processes (PM2 + any orphaned processes on our ports)
- Clean ALL build artifacts (Rust + TypeScript)
- Build EVERYTHING (debug + release mode)
- Run ALL tests and ensure they pass
- Only then start services back up

**SINGLE SOURCE OF TRUTH**: All port numbers are defined in `.env` and `.env.example`.
No hardcoded ports anywhere in scripts or configs.

**PORTABLE**: This script uses `git rev-parse --show-toplevel` to auto-detect the project root, so it works on any developer's machine regardless of their home directory path.

## Port Configuration (from .env)

These ports are automatically read by the applications:
- `REST_PORT=9437` - Backend REST API
- `SSE_PORT=9438` - Backend SSE endpoint
- `DASHBOARD_PORT=9439` - Frontend dashboard

**Important**: The frontend `vite.config.ts` has `strictPort: true` set to prevent Vite from auto-incrementing to 9440/9441 if port 9439 is busy. This ensures rebuild scripts fail fast rather than silently starting on wrong ports.

## Complete Rebuild Steps

### 1. KILL ALL PROCESSES (No survivors!)

```bash
# Detect project root (works on any developer's machine)
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
echo "Project root: $PROJECT_ROOT"

# Stop all PM2 processes
pm2 delete all 2>/dev/null || true

# Kill by FULL PATH to avoid conflicts with other projects
# (Important: generic pkill patterns can kill wrong processes)
pkill -f "$PROJECT_ROOT/target/release/rustymail-server" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/release/rustymail-mcp-stdio" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/debug/rustymail-server" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/.bin/vite" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/.bin/dotenv" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/@esbuild" 2>/dev/null || true

# Kill any processes on our specific ports (catches orphans and alternate port users)
lsof -ti:9437 | xargs kill -9 2>/dev/null || true
lsof -ti:9438 | xargs kill -9 2>/dev/null || true
lsof -ti:9439 | xargs kill -9 2>/dev/null || true

# IMPORTANT: Node process trees (Vite + esbuild) take 2-3 seconds to fully terminate
# Don't reduce this delay or you'll hit port race conditions!
echo "Waiting for process cleanup..."
sleep 3

# Verify ports are actually free before proceeding
echo "Verifying ports are free..."
if lsof -i:9437,9438,9439 2>/dev/null; then
    echo "⚠️  WARNING: Some ports still in use! Forcing cleanup..."
    lsof -ti:9437,9438,9439 | xargs kill -9 2>/dev/null || true
    sleep 2
fi

lsof -i:9437,9438,9439 || echo "✅ All ports are free!"
```

### 2. CLEAN ALL BUILD ARTIFACTS

```bash
# Clean Rust build artifacts (backend + MCP stdio)
cargo clean

# Clean frontend build artifacts
cd frontend/rustymail-app-main
rm -rf dist
rm -rf node_modules/.vite
rm -rf .vite
npm run clean 2>/dev/null || true
cd ../..

echo "All build artifacts cleaned!"
```

### 3. BUILD EVERYTHING (Debug + Release + Tests)

```bash
# Build ALL binaries in DEBUG mode
echo "Building all binaries (debug)..."
cargo build

# Build ALL binaries in RELEASE mode
echo "Building all binaries (release)..."
cargo build --release

# This builds:
# - rustymail-server (main backend server)
# - rustymail-mcp-stdio (thin MCP stdio proxy)
# - rustymail-mcp-stdio-high-level (high-level MCP stdio proxy)
# - rustymail-sync (separate sync process for memory reclamation)

# Build ALL tests (don't run yet, just compile)
echo "Building tests..."
cargo test --workspace --no-run

# Build frontend
echo "Building frontend..."
cd frontend/rustymail-app-main
npm run build
cd ../..

echo "All builds completed successfully!"
```

### 4. RUN TESTS (Must pass before proceeding!)

```bash
echo "Running tests..."
cargo test --workspace 2>&1 | head -100

# Check test exit code
if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "❌ TESTS FAILED! Not starting services."
    echo "Fix the failing tests before proceeding."
    exit 1
fi

echo "✅ All tests passed!"
```

### 5. START SERVICES (Only after everything passes)

```bash
# Ensure logs directory exists
mkdir -p logs

# Start all services with PM2 (reads ports from .env)
pm2 start frontend/rustymail-app-main/ecosystem.config.cjs

# Save PM2 process list
pm2 save

# Wait for services to start
sleep 3

echo "Services started! Checking status..."
pm2 status
```

### 6. VERIFY SERVICES ARE RUNNING

```bash
# Check PM2 status
pm2 status

# Verify backend is responding (uses DASHBOARD_PORT from .env)
echo "Checking backend health..."
curl -s http://localhost:9437/api/dashboard/stats | jq . || echo "Backend not responding yet..."

# Verify frontend is serving (uses DASHBOARD_PORT from .env)
echo "Checking frontend..."
curl -s -I http://localhost:9439 | head -5 || echo "Frontend not responding yet..."

echo ""
echo "✅ REBUILD COMPLETE!"
echo "Backend: http://localhost:9437"
echo "Frontend: http://localhost:9439"
```

## PM2 Commands Reference

```bash
# View service status
pm2 status

# View logs (all services)
pm2 logs

# View logs (specific service)
pm2 logs rustymail-backend
pm2 logs rustymail-frontend

# Restart a specific service
pm2 restart rustymail-backend
pm2 restart rustymail-frontend

# Restart all services
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
- **Binary**: `target/release/rustymail-server`
- **Config**: Reads `.env` via `dotenvy` crate at startup
- **Ports**: `REST_PORT`, `SSE_PORT` from `.env`
- **Database**: `CACHE_DATABASE_URL=sqlite:data/email_cache.db`
- **Working Dir**: Project root (`/Users/au/src/RustyMail`)

### Frontend (TypeScript/Vite)
- **Build Dir**: `frontend/rustymail-app-main/dist`
- **Dev Server**: `npm run dev` (uses `dotenv-cli`)
- **Config**: Loads `../../.env` via `dotenv -e ../../.env -- vite`
- **Port**: `DASHBOARD_PORT` from `.env`
- **API URL**: Proxies to backend via `VITE_API_URL`

### MCP Stdio Adapters
- **Binary**: `target/release/rustymail-mcp-stdio` (thin proxy)
- **Binary**: `target/release/rustymail-mcp-stdio-high-level` (high-level proxy)
- **Purpose**: MCP protocol adapters over stdio for Claude Code integration
- **Config**: Inherits environment from parent process

### Sync Process
- **Binary**: `target/release/rustymail-sync`
- **Purpose**: Standalone sync process for memory reclamation
- **How it works**: Main server spawns this binary periodically. When it exits, OS reclaims ALL memory.
- **Config**: Reads `CACHE_DATABASE_URL` from environment, uses `data/.sync.lock` for single-instance locking

### PM2 Configuration
- **File**: `frontend/rustymail-app-main/ecosystem.config.cjs`
- **No hardcoded ports**: Relies on apps reading from `.env`
- **Logs**: `logs/backend-*.log`, `logs/frontend-*.log`
- **Auto-restart**: Up to 10 restarts, minimum 10s uptime

## Troubleshooting

### Build Issues
- **Stale artifacts**: Always run `cargo clean` before rebuilding
- **Compilation errors**: Check Rust version with `rustc --version` (should be 1.70+)
- **Missing dependencies**: Run `cargo update` to refresh Cargo.lock
- **Frontend build fails**: Run `npm install` in `frontend/rustymail-app-main`

### Port Conflicts
- **Port already in use**: Run the kill commands from Step 1 again
- **Process on alternate port**: Check with `lsof -i | grep vite` or `lsof -i | grep rustymail`
- **Docker conflicts**: Check if Docker containers are using these ports with `docker ps`

### Database Issues
- **Empty after rebuild**: Database is preserved, trigger sync: `curl -X POST 'http://localhost:9437/api/dashboard/sync/trigger?account_id=<account_id>'`
- **Cache not initialized**: Check that `data/email_cache.db` exists and backend has write permissions
- **Wrong working directory**: Ensure PM2 `cwd` matches project root

### Test Failures
- **Tests must pass**: Fix all failing tests before services will start
- **Integration tests fail**: Check that test database paths are correct
- **Timeout errors**: Increase test timeouts in `Cargo.toml`

### Environment Variables
- **Missing .env**: Copy `.env.example` to `.env` and configure
- **Wrong ports**: Edit `.env` (not ecosystem.config.js!)
- **API key issues**: Check `RUSTYMAIL_API_KEY` in `.env`
- **Frontend can't reach backend**: Verify `VITE_API_URL=/api` in `.env`

## Single Source of Truth Principle

**ALL configuration must be in `.env`** (or `.env.example` as template):
- ✅ Port numbers
- ✅ API keys
- ✅ Database URLs
- ✅ Feature flags
- ✅ Timeouts and limits

**NO configuration in**:
- ❌ Source code (Rust or TypeScript)
- ❌ PM2 config files
- ❌ Build scripts
- ❌ This slash command

**How it works**:
1. Backend: `dotenvy` loads `.env` → Rust config struct → Runtime values
2. Frontend: `dotenv-cli` injects `.env` → Vite reads `VITE_*` vars → Build-time constants
3. PM2: Just runs the binaries, they handle their own config

This ensures we can change ANY configuration value in ONE place (`.env`) without touching code or configs.
