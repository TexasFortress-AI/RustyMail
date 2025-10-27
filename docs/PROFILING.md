# Memory Profiling Guide for RustyMail

This document explains how to profile and monitor memory usage in RustyMail to detect and diagnose memory leaks.

## Quick Start

The easiest way to profile is using the provided script:

```bash
# Simple real-time memory monitor
./scripts/profile-memory.sh 1

# Extended 30-minute monitoring with statistics
./scripts/profile-memory.sh 2

# macOS Instruments - Allocations profiling
./scripts/profile-memory.sh 3

# macOS Instruments - Leaks detection
./scripts/profile-memory.sh 4

# Process memory map analysis
./scripts/profile-memory.sh 5
```

## Profiling Methods

### 1. Simple Memory Monitor (Recommended First Step)

**Best for**: Quick checks, real-time monitoring during development

```bash
./scripts/profile-memory.sh 1
```

Shows:
- Process ID
- Memory usage (MB)
- CPU usage (%)
- Updates every 2 seconds

Press Ctrl+C to stop.

### 2. Extended Memory Monitor (30 Minutes)

**Best for**: Detecting slow memory leaks, baseline measurements

```bash
./scripts/profile-memory.sh 2
```

Features:
- Tracks memory every 60 seconds for 30 minutes
- Saves results to `logs/memory-profile-YYYYMMDD-HHMMSS.log`
- Calculates average, peak, and growth statistics
- CSV format for easy analysis in Excel/spreadsheets

Example output:
```
Average Memory: 52.3 MB
Peak Memory: 54.1 MB
Memory Growth: 1.8 MB
```

### 3. macOS Instruments - Allocations

**Best for**: Finding where memory is being allocated, heap growth analysis

**Requirements**: Xcode Command Line Tools (`xcode-select --install`)

```bash
./scripts/profile-memory.sh 3
```

This opens Xcode Instruments with the Allocations template. You can:
- See live heap growth graphs
- Identify which types are consuming most memory
- View allocation backtraces
- Export detailed reports

**Usage tips**:
1. Let it run for 5-10 minutes while using the application
2. Look for continuously growing allocations (red flag!)
3. Click on large allocations to see stack traces
4. Save the trace file for later analysis

### 4. macOS Instruments - Leaks

**Best for**: Detecting memory that's allocated but never freed

**Requirements**: Xcode Command Line Tools

```bash
./scripts/profile-memory.sh 4
```

This opens Xcode Instruments with the Leaks template. It will:
- Automatically scan for leaked memory
- Show leak graphs over time
- Provide stack traces for each leak
- Categorize leaks by type

**What to look for**:
- Red markers indicate detected leaks
- Steady leak count = no new leaks (good!)
- Growing leak count = active leak (investigate!)

### 5. Process Memory Map

**Best for**: Understanding overall memory layout, identifying unusual allocations

```bash
./scripts/profile-memory.sh 5
```

Shows:
- Memory regions and their sizes
- Heap, stack, and shared library mappings
- Total virtual and resident memory
- Detailed breakdown by region type

## DHAT Heap Profiling (Advanced)

DHAT is Rust's built-in heap profiler that generates detailed allocation reports.

### Setup

1. Add DHAT initialization to `src/main.rs`:

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // ... rest of main
}
```

2. Build and run with DHAT enabled:

```bash
cargo build --release --features dhat-heap --bin rustymail-server
./target/release/rustymail-server
```

3. Use the application for a while, then stop it (Ctrl+C)

4. DHAT will generate `dhat-heap.json` in the current directory

5. View the report:

```bash
# Open in browser
open https://nnethercote.github.io/dh_view/dh_view.html

# Upload the dhat-heap.json file
```

### What DHAT Shows

- **Total bytes allocated**: How much memory was allocated in total
- **Max bytes**: Peak heap usage
- **At t-gmax**: Memory state at peak usage
- **At t-end**: Memory state when program ended
- **Allocation backtraces**: Where allocations came from

**Red flags**:
- "At t-end" much larger than expected = likely leak
- Specific allocations growing without bounds
- Blocks allocated but never freed

## Common Memory Leak Patterns in Rust

### 1. Circular Arc References

**Problem**: Two Arc pointers reference each other, preventing drop

```rust
// BAD - Creates circular reference
struct A { b: Arc<B> }
struct B { a: Arc<A> }

// GOOD - Use Weak for back-references
struct A { b: Arc<B> }
struct B { a: Weak<A> }
```

**Fixed in RustyMail**: src/dashboard/services/metrics.rs (passed only connection_pool instead of entire DashboardState)

### 2. Unbounded Collections

**Problem**: Collections that grow forever without cleanup

```rust
// BAD - Grows forever
static CACHE: Mutex<HashMap<String, Data>> = ...;

// GOOD - Use LRU cache with size limit
static CACHE: Mutex<LruCache<String, Data>> = ...;
```

**Fixed in RustyMail**:
- src/dashboard/services/cache.rs (LruCache with 100 item limit)
- src/connection_pool.rs (periodic queue rebuild)

### 3. Forgotten Background Tasks

**Problem**: Spawned tasks that accumulate data

```rust
// BAD - Accumulates data indefinitely
tokio::spawn(async {
    loop {
        data.push(fetch_something().await);
    }
});

// GOOD - Prune old data
tokio::spawn(async {
    loop {
        data.push(fetch_something().await);
        // Remove items older than 1 hour
        data.retain(|x| x.age < Duration::hours(1));
    }
});
```

## Interpreting Results

### Healthy Memory Profile
- Memory usage stabilizes after initial startup (typically < 100 MB for RustyMail)
- Minor fluctuations (±10 MB) are normal
- No sustained upward trend over hours

### Signs of Memory Leak
- **Steady growth**: Memory increases continuously over time
- **Never stabilizes**: Keeps growing even when idle
- **Proportional to activity**: Memory grows with each request/operation

### Example Analysis

```
Time    Memory  Analysis
-----   ------  ---------
0:00    47 MB   Initial startup
0:30    52 MB   Normal growth (caches warming up)
1:00    54 MB   Stabilizing
2:00    54 MB   ✓ HEALTHY - Stable for 1 hour
6:00    55 MB   ✓ HEALTHY - Minimal growth

vs.

0:00    47 MB   Initial startup
0:30    65 MB   Growing quickly
1:00    91 MB   ⚠️ WARNING - Continuous growth
2:00    143 MB  🔴 LEAK - Unbounded growth
6:00    387 MB  🔴 CRITICAL - Investigate immediately
```

## Known Memory Leak Fixes

RustyMail has fixed these memory leaks (2025-01-25):

1. **Circular Arc in Metrics Service** (80 GB leak)
   - File: src/dashboard/services/metrics.rs:186
   - Fix: Pass only Arc<ConnectionPool> to background task instead of Arc<DashboardState>

2. **Connection Pool UUID Queue**
   - File: src/connection_pool.rs
   - Fix: Periodic queue rebuild to remove stale UUIDs

3. **Folder Cache Unbounded Growth**
   - File: src/dashboard/services/cache.rs
   - Fix: Replaced HashMap with LruCache (100 item limit)

## Troubleshooting

### "instruments: command not found"

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

### DHAT not generating output

Make sure you:
1. Built with `--features dhat-heap`
2. Actually used the application (triggered some operations)
3. Cleanly shut down the server (Ctrl+C, not kill -9)
4. Check current directory for `dhat-heap.json`

### PM2 interfering with profiling

Stop PM2 before using Instruments:
```bash
pm2 stop all
./scripts/profile-memory.sh 3
pm2 start all
```

## Recommended Workflow

1. **Quick check**: Run simple monitor for 2-3 minutes during testing
   ```bash
   ./scripts/profile-memory.sh 1
   ```

2. **Baseline measurement**: Run extended monitor overnight or during heavy usage
   ```bash
   ./scripts/profile-memory.sh 2
   ```

3. **If leak suspected**: Use Instruments Allocations to find growing allocations
   ```bash
   ./scripts/profile-memory.sh 3
   ```

4. **Deep dive**: Add DHAT profiling for detailed allocation analysis
   ```bash
   cargo build --release --features dhat-heap
   # Run and generate report
   ```

## Performance Impact

- **Simple/Extended Monitor**: Negligible (<0.1% overhead)
- **Instruments**: ~5-10% overhead, don't use in production
- **DHAT**: ~10-20% overhead, development/testing only

## Additional Resources

- [Rust Performance Book - Memory Profiling](https://nnethercote.github.io/perf-book/profiling.html)
- [DHAT Documentation](https://docs.rs/dhat/)
- [Instruments User Guide](https://help.apple.com/instruments/)
- [Understanding Memory Issues in Rust](https://blog.rust-lang.org/2021/02/26/const-generics.html)

## Questions?

If you discover new memory leaks or have questions about profiling:
1. Check this guide first
2. Run the profiling tools
3. Document the leak pattern
4. File an issue with profiling data
