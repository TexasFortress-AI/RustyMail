# Complete Clean - Shutdown and Build Cleanup

Perform a complete clean shutdown and remove all build artifacts.

## Steps to Execute:

### 1. Stop all running processes

```bash
# Detect project root (works on any developer's machine)
PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
echo "Project root: $PROJECT_ROOT"

# Stop all PM2 processes
pm2 delete all 2>/dev/null || true

# Kill by FULL PATH to avoid conflicts with other projects
pkill -f "$PROJECT_ROOT/target/release/rustymail-server" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/release/rustymail-mcp-stdio" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/release/rustymail-mcp-stdio-high-level" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/release/rustymail-sync" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/debug/rustymail-server" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/target/debug/rustymail-sync" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/.bin/vite" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/.bin/dotenv" 2>/dev/null || true
pkill -f "$PROJECT_ROOT/frontend/rustymail-app-main/node_modules/@esbuild" 2>/dev/null || true

# Kill any processes on our specific ports
lsof -ti:9437 | xargs kill -9 2>/dev/null || true
lsof -ti:9438 | xargs kill -9 2>/dev/null || true
lsof -ti:9439 | xargs kill -9 2>/dev/null || true

echo "Waiting for process cleanup..."
sleep 3

# Verify ports are free
lsof -i:9437,9438,9439 || echo "All ports are free!"
```

### 2. Clean Rust/Cargo artifacts

```bash
# Full cargo clean removes ~2-5 GB of build artifacts
cargo clean

# Optionally remove Cargo.lock (will be regenerated)
# rm Cargo.lock
```

### 3. Clean Frontend artifacts

```bash
cd frontend/rustymail-app-main

# Remove built files
rm -rf dist
rm -rf node_modules/.vite
rm -rf .vite

# Optional: remove node_modules entirely (~500MB)
# rm -rf node_modules

cd ../..
```

### 4. Clean temporary and log files

```bash
# Clear log files
rm -f logs/*.log 2>/dev/null || true

# Clear sync lock file
rm -f data/.sync.lock 2>/dev/null || true

# Clear editor swap files
find . -name "*.swp" -delete 2>/dev/null || true
find . -name "*.swo" -delete 2>/dev/null || true
find . -name "*~" -delete 2>/dev/null || true
```

### 5. Verify cleanup

```bash
# Check what remains
echo "Remaining target directory size:"
du -sh target 2>/dev/null || echo "target/ directory removed"

echo "Remaining node_modules size:"
du -sh frontend/rustymail-app-main/node_modules 2>/dev/null || echo "node_modules exists"

# Verify no processes running
pm2 status
lsof -i:9437,9438,9439 || echo "All ports are free!"
```

## Important Notes:

- **Database files (data/*.db) are NOT deleted** - these contain your cached emails
- **Configuration files (.env, config/*.json) are preserved**
- **Source code is untouched**
- After cleanup, you'll need to rebuild everything from scratch

## Binaries that will be rebuilt:

- `rustymail-server` - Main backend server
- `rustymail-mcp-stdio` - Thin MCP stdio proxy
- `rustymail-mcp-stdio-high-level` - High-level MCP stdio proxy
- `rustymail-sync` - Standalone sync process (for memory reclamation)

## Expected Outcome:

- All processes stopped
- Build directories cleaned (~2-5 GB freed typically)
- Fresh state ready for `cargo build` and `npm install`
- No running services

Execute this command when you want a completely fresh build or are troubleshooting build issues.
