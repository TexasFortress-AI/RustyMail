# CI/CD Integration Guide for RustyMail E2E Tests

## Overview

This document describes the CI/CD integration for RustyMail's comprehensive end-to-end test suite (Task 69.7). The pipeline automates testing across MCP protocol compliance, dashboard UI functionality, and complete email workflows.

## GitHub Actions Workflow

The main workflow is defined in `.github/workflows/e2e-tests.yml`.

### Workflow Triggers

```yaml
on:
  push:
    branches: [ main, develop ]      # Run on commits to main branches
  pull_request:
    branches: [ main ]               # Run on PRs to main
  workflow_dispatch:                  # Allow manual triggering
```

### Job Structure

```
┌─────────────────────────────────────────────────────────┐
│                    e2e-tests Job                        │
├─────────────────────────────────────────────────────────┤
│  1. Setup Environment                                   │
│     - Checkout code                                     │
│     - Setup Rust, Node.js, Python                       │
│     - Cache dependencies                                │
│                                                         │
│  2. Install Dependencies                                │
│     - Python packages (MCP SDK)                         │
│     - Frontend npm packages                             │
│     - Build Rust binaries                               │
│                                                         │
│  3. Run Tests                                          │
│     - Unit tests (cargo test --lib)                    │
│     - Integration tests (cargo test --test)            │
│                                                         │
│  4. Start Services                                     │
│     - Backend server (:9437)                           │
│     - Frontend server (:5173)                          │
│     - Health checks                                    │
│                                                         │
│  5. E2E Tests                                          │
│     - MCP client protocol tests                        │
│     - Dashboard UI workflow tests                      │
│     - Complete workflow integration tests              │
│                                                         │
│  6. Cleanup & Reporting                                │
│     - Stop servers                                     │
│     - Collect artifacts                                │
│     - Generate test report                             │
│     - Upload results                                   │
└─────────────────────────────────────────────────────────┘
```

## Test Execution Order

### Phase 1: Compilation & Unit Tests (Fast Feedback)
```bash
cargo build --release --bin rustymail-server
cargo build --release --bin rustymail-mcp-stdio
cargo test --lib --bins
```

**Duration:** ~2-3 minutes
**Purpose:** Catch compilation errors and unit test failures early

### Phase 2: Integration Tests
```bash
cargo test --test '*' -- --test-threads=1
```

**Duration:** ~5-10 minutes
**Purpose:** Validate component integration and HTTP endpoints

### Phase 3: Service Startup
```bash
# Backend
./target/release/rustymail-server &
# Health check: curl http://localhost:9437/api/dashboard/status

# Frontend
npm run dev &
# Health check: curl http://localhost:5173
```

**Duration:** ~30 seconds
**Purpose:** Prepare environment for E2E tests

### Phase 4: E2E Test Execution
```bash
# MCP client tests
python scripts/test_mcp_client.py --transport stdio

# UI workflow tests (if available)
python scripts/test_ui_workflows.py --url http://localhost:5173
```

**Duration:** ~5-10 minutes
**Purpose:** Comprehensive end-to-end validation

## Environment Configuration

### Required Environment Variables

```bash
# MCP Configuration
export MCP_BACKEND_URL=http://localhost:9437/mcp
export MCP_TIMEOUT=30

# Rust Configuration
export RUST_BACKTRACE=1
export CARGO_TERM_COLOR=always

# Test Database
export RUSTYMAIL_TEST_MODE=true
export DATABASE_PATH=.rustymail/test_cache.db
```

### GitHub Secrets (if needed)

For future integration with external services:

```yaml
secrets:
  IMAP_TEST_USERNAME: ${{ secrets.IMAP_TEST_USERNAME }}
  IMAP_TEST_PASSWORD: ${{ secrets.IMAP_TEST_PASSWORD }}
```

## Artifact Collection

### Collected Artifacts

1. **Server Logs**
   - `backend.log` - Backend server output
   - `frontend.log` - Frontend server output

2. **Test Results**
   - `test-results/workflow-report.json` - Workflow test results
   - `test-results/mcp-client-results.txt` - MCP client test output

3. **Screenshots** (if UI tests run)
   - `test-results/screenshots/*.png` - Puppeteer screenshots

4. **Performance Metrics**
   - `test-results/performance.json` - Timing data

### Artifact Retention

```yaml
retention-days: 7  # Keep artifacts for 1 week
```

### Viewing Artifacts

1. Navigate to GitHub Actions run
2. Scroll to bottom of run page
3. Download "e2e-test-artifacts" ZIP file

## Failure Handling

### Automatic Cleanup

The workflow ensures cleanup even on failure:

```yaml
- name: Stop servers
  if: always()  # Runs even if previous steps fail
  run: |
    kill $(cat backend.pid) || true
    kill $(cat frontend.pid) || true
```

### Debug Information

On failure, the workflow provides:

1. **Server logs** uploaded as artifacts
2. **GitHub Step Summary** with status indicators
3. **Exit codes** from each test phase

### Common Failure Scenarios

#### Scenario 1: Backend Startup Failure

**Symptoms:**
```
Backend server failed to start
curl: (7) Failed to connect to localhost port 9437
```

**Debug:**
1. Check `backend.log` artifact
2. Verify port not already in use
3. Check database migration issues

**Fix:**
- Ensure test database directory exists: `mkdir -p .rustymail`
- Verify no conflicting processes
- Check migration scripts

#### Scenario 2: Frontend Build Failure

**Symptoms:**
```
Frontend server failed to start
npm ERR! code ELIFECYCLE
```

**Debug:**
1. Check `frontend.log` artifact
2. Verify npm dependencies installed
3. Check for TypeScript errors

**Fix:**
- Run `npm ci` instead of `npm install`
- Check `package-lock.json` is committed
- Verify Node.js version compatibility

#### Scenario 3: MCP Client Test Failure

**Symptoms:**
```
✗ Some tests failed
AssertionError: Tool count expected 18, got 17
```

**Debug:**
1. Check test output in workflow logs
2. Verify MCP tool registrations
3. Check stdio proxy connectivity

**Fix:**
- Verify all 18 tools registered in `mcp_tools/mod.rs`
- Check stdio proxy environment variables
- Verify backend MCP endpoint responding

## Performance Optimization

### Caching Strategy

```yaml
- name: Cache Cargo dependencies
  uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/bin/
      ~/.cargo/registry/index/
      ~/.cargo/registry/cache/
      ~/.cargo/git/db/
      target/
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

**Benefit:** Reduces build time from ~10min to ~2min on cache hit

### Parallel Execution

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest]
    rust: [stable, nightly]
```

**Benefit:** Test across multiple environments simultaneously

### Test Isolation

```bash
cargo test --test '*' -- --test-threads=1
```

**Reason:** Prevents port conflicts and database contention in E2E tests

## Local Testing

### Run Full CI/CD Pipeline Locally

```bash
# Install act (GitHub Actions local runner)
brew install act  # macOS
# or
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash  # Linux

# Run E2E workflow
act -j e2e-tests
```

### Run Individual Test Phases

```bash
# Phase 1: Build & Unit Tests
cargo build --release --bin rustymail-server
cargo test --lib --bins

# Phase 2: Integration Tests
cargo test --test '*' -- --test-threads=1

# Phase 3: Start Services
./target/release/rustymail-server &
cd frontend && npm run dev &

# Phase 4: E2E Tests
source .venv/bin/activate
python scripts/test_mcp_client.py --transport stdio
```

### Quick Validation Script

```bash
#!/bin/bash
# scripts/validate_cicd.sh

echo "Validating CI/CD readiness..."

# Check required files exist
files=(
    ".github/workflows/e2e-tests.yml"
    "scripts/test_mcp_client.py"
    "scripts/run_workflow_tests.sh"
    "scripts/requirements.txt"
    "frontend/package.json"
)

for file in "${files[@]}"; do
    if [ ! -f "$file" ]; then
        echo "❌ Missing: $file"
        exit 1
    fi
    echo "✅ Found: $file"
done

# Check binaries build
if cargo build --release --bin rustymail-server; then
    echo "✅ Backend builds successfully"
else
    echo "❌ Backend build failed"
    exit 1
fi

# Check Python dependencies
if source .venv/bin/activate && python -c "import mcp; import dotenv"; then
    echo "✅ Python dependencies installed"
else
    echo "❌ Python dependencies missing"
    exit 1
fi

echo ""
echo "✅ CI/CD validation passed!"
```

## Integration with Other CI/CD Platforms

### GitLab CI

```yaml
# .gitlab-ci.yml
e2e-tests:
  stage: test
  image: rust:latest
  services:
    - name: postgres:14
      alias: database
  variables:
    MCP_BACKEND_URL: "http://localhost:9437/mcp"
    MCP_TIMEOUT: "30"
  before_script:
    - apt-get update && apt-get install -y nodejs npm python3 python3-venv
    - cargo build --release --bin rustymail-server
  script:
    - bash scripts/run_workflow_tests.sh
  artifacts:
    when: always
    paths:
      - test-results/
    expire_in: 1 week
```

### Jenkins Pipeline

```groovy
// Jenkinsfile
pipeline {
    agent any

    environment {
        MCP_BACKEND_URL = 'http://localhost:9437/mcp'
        MCP_TIMEOUT = '30'
    }

    stages {
        stage('Build') {
            steps {
                sh 'cargo build --release --bin rustymail-server'
                sh 'cd frontend && npm ci'
            }
        }

        stage('Test') {
            steps {
                sh 'cargo test --all'
            }
        }

        stage('E2E Tests') {
            steps {
                sh 'bash scripts/run_workflow_tests.sh'
            }
        }
    }

    post {
        always {
            archiveArtifacts artifacts: 'test-results/**/*', allowEmptyArchive: true
            junit 'test-results/**/*.xml'
        }
    }
}
```

### CircleCI

```yaml
# .circleci/config.yml
version: 2.1

executors:
  rust-executor:
    docker:
      - image: rust:latest
    working_directory: ~/project

jobs:
  e2e-tests:
    executor: rust-executor
    steps:
      - checkout
      - run:
          name: Install dependencies
          command: |
            apt-get update && apt-get install -y nodejs npm python3 python3-venv
      - run:
          name: Build backend
          command: cargo build --release --bin rustymail-server
      - run:
          name: Run E2E tests
          command: bash scripts/run_workflow_tests.sh
      - store_artifacts:
          path: test-results
      - store_test_results:
          path: test-results

workflows:
  version: 2
  test-workflow:
    jobs:
      - e2e-tests
```

## Monitoring & Metrics

### Test Duration Tracking

Track test execution time to identify performance regressions:

```bash
# Add to workflow
- name: Track test duration
  run: |
    echo "test_duration_seconds{phase=\"unit\"} $(duration)" >> metrics.txt
    echo "test_duration_seconds{phase=\"integration\"} $(duration)" >> metrics.txt
    echo "test_duration_seconds{phase=\"e2e\"} $(duration)" >> metrics.txt
```

### Success Rate Dashboard

Create a dashboard showing:
- Test pass rate over time
- Most frequent failure points
- Average test duration
- Flaky test detection

### Slack/Discord Notifications

```yaml
- name: Notify on failure
  if: failure()
  uses: 8398a7/action-slack@v3
  with:
    status: ${{ job.status }}
    text: 'E2E tests failed on ${{ github.ref }}'
    webhook_url: ${{ secrets.SLACK_WEBHOOK }}
```

## Future Enhancements

### Planned Improvements

1. **Parallel Test Execution**
   - Split E2E tests into independent jobs
   - Run MCP client tests + UI tests concurrently
   - Expected time savings: 30-40%

2. **Visual Regression Testing**
   - Add Percy.io or similar integration
   - Capture screenshots automatically
   - Alert on unexpected visual changes

3. **Performance Benchmarking**
   - Track API response times
   - Monitor memory usage
   - Alert on performance regressions

4. **Scheduled Nightly Runs**
   - Extended test suite with stress tests
   - Multi-account concurrent testing
   - Load testing with 1000+ emails

5. **Browser Matrix Testing**
   - Test UI on Chrome, Firefox, Safari
   - Mobile responsive testing
   - Cross-browser compatibility validation

## Success Criteria

Task 69.7 is complete when:
- [x] GitHub Actions workflow created
- [x] Service startup/shutdown automated
- [x] Test execution orchestrated
- [x] Artifact collection configured
- [x] Failure handling implemented
- [x] Documentation complete
- [x] Local testing validated
- [x] CI/CD integration patterns documented

## Related Tasks

- Task 69.1: MCP Inspector setup ✅
- Task 69.2: Python MCP client ✅
- Task 69.3: Protocol compliance ✅
- Task 69.4: Puppeteer configuration ✅
- Task 69.5: Dashboard UI tests ✅
- Task 69.6: Workflow integration tests ✅
- Task 69.7: CI/CD integration ✅ (this task)
