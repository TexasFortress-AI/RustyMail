# kill-and-restart-backend.sh

Script for safely restarting the RustyMail backend server with automatic memory leak analysis.

## What It Does

1. **Before Shutdown** (if server is running):
   - Records current memory usage
   - Runs macOS `leaks` tool to detect memory leaks
   - Captures `vmmap` memory map summary
   - Saves reports to `logs/memory-profiles/`

2. **Restart**:
   - Stops the PM2 process
   - Kills any orphaned processes
   - Rebuilds the server (release mode)
   - Starts the server via PM2

3. **After Restart**:
   - Verifies server started successfully
   - Shows new PID and initial memory
   - Compares memory usage before/after restart

## Usage

```bash
./scripts/kill-and-restart-backend.sh
```

## Output Files

Memory profile logs are saved to `logs/memory-profiles/`:

- `leaks-YYYYMMDD-HHMMSS.txt` - Memory leak analysis (only if leaks detected)
- `vmmap-YYYYMMDD-HHMMSS.txt` - Memory map summary

## Interpreting Results

### Memory Leaks
If leaks are detected, the report shows:
- Total bytes leaked
- Number of leak instances
- Stack traces for allocation sites

### Memory Growth
Compare "Before restart" vs "After restart" memory:
- If before >> after, the server had accumulated memory over time
- Consistent growth suggests a memory leak
- Track this over multiple restarts to identify patterns

## Troubleshooting

### "Could not run leaks analysis"
The `leaks` tool may require:
- Running with `sudo`
- Disabling System Integrity Protection (SIP) on macOS
- Granting debugging permissions

### Server not starting
Check PM2 logs:
```bash
pm2 logs rustymail-backend --lines 50
```

## Related Scripts

- `scripts/profile-memory.sh` - Interactive memory profiling options
- `scripts/run-dhat-profiling.sh` - DHAT heap profiling for detailed analysis
