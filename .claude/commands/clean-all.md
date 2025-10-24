# Complete Clean - Shutdown and Build Cleanup

Perform a complete clean shutdown and remove all build artifacts.

## Steps to Execute:

1. **Stop all running processes:**
   - Stop PM2 processes (rustymail-backend, rustymail-frontend)
   - Kill any orphaned processes on ports 9437 and 9439

2. **Clean Rust/Cargo artifacts:**
   - Run `cargo clean` to remove all target directory contents
   - Remove Cargo.lock (will be regenerated on next build)

3. **Clean Frontend artifacts:**
   - Remove node_modules in frontend directory
   - Remove dist/build folders
   - Clear npm/vite cache

4. **Clean temporary files:**
   - Remove log files in logs/ directory
   - Clear any .swp, .tmp, or backup files

5. **Verify cleanup:**
   - Check disk space freed
   - List remaining artifacts
   - Confirm all processes stopped

## Important Notes:

- **Database files (data/*.db) are NOT deleted** - these contain your cached emails
- **Configuration files (.env, config/*.json) are preserved**
- **Source code is untouched**
- After cleanup, you'll need to rebuild everything from scratch

## Expected Outcome:

- All processes stopped
- Build directories cleaned (~2-5 GB freed typically)
- Fresh state ready for `cargo build` and `npm install`
- No running services

Execute this command when you want a completely fresh build or are troubleshooting build issues.
