# Task ID: 29

**Title:** Pin git dependencies to specific commit SHAs

**Status:** done

**Dependencies:** 32 ✓

**Priority:** low

**Description:** Update Cargo.toml to pin all git dependencies (including rmcp crate) to specific commit SHAs to prevent supply chain attacks, document the pinned versions, and establish a periodic review process for dependency updates.

**Details:**

Implement secure dependency pinning to protect against supply chain attacks by ensuring all git dependencies reference immutable commit SHAs:

1) **Audit current git dependencies in Cargo.toml**:
   - Search for all dependencies using git = "..." syntax
   - Identify the rmcp crate and any other git-based dependencies
   - For each dependency, determine the current branch/tag being tracked
   - Clone each repository and identify the exact commit SHA currently in use

2) **Pin dependencies to specific commit SHAs**:
   - Replace branch/tag references with rev = "SHA" for each git dependency
   - Example transformation:
     ```toml
     # Before (vulnerable to upstream changes):
     rmcp = { git = "https://github.com/example/rmcp", branch = "main" }
     
     # After (pinned to specific commit):
     rmcp = { git = "https://github.com/example/rmcp", rev = "a1b2c3d4e5f6..." }
     ```
   - Run `cargo update` to ensure the lock file reflects the pinned versions
   - Verify the application builds and tests pass with pinned dependencies

3) **Document pinned dependencies**:
   - Create docs/dependency-pins.md with a table documenting:
     - Dependency name
     - Repository URL
     - Pinned commit SHA
     - Commit date and author
     - Version/tag the commit corresponds to (if any)
     - Brief description of why this specific commit was chosen
     - Last review date
   - Add comments in Cargo.toml above each pinned dependency explaining the version

4) **Establish review process**:
   - Create .github/workflows/dependency-review.yml for monthly automated checks:
     ```yaml
     name: Dependency Review
     on:
       schedule:
         - cron: '0 0 1 * *'  # Monthly on the 1st
       workflow_dispatch:
     
     jobs:
       review-git-deps:
         runs-on: ubuntu-latest
         steps:
           - uses: actions/checkout@v3
           - name: Check for updates
             run: |
               # Script to check each pinned repo for new commits
               # Create issues for dependencies with updates available
     ```
   - Add a SECURITY.md section on dependency update procedures
   - Document the review checklist:
     - Check upstream repository for security advisories
     - Review commit history since pinned version
     - Test updates in isolated environment
     - Update both Cargo.toml and docs/dependency-pins.md

5) **Add CI validation**:
   - Create a GitHub Action that fails if any git dependencies lack rev pins
   - Add pre-commit hook to warn developers about unpinned dependencies
   - Include dependency pinning in security audit checklist

**Test Strategy:**

Verify the dependency pinning implementation with comprehensive testing:

1) **Validate all git dependencies are pinned**:
   - Parse Cargo.toml and verify every git dependency has a `rev` field
   - Ensure no git dependencies use `branch`, `tag`, or default to HEAD
   - Run `cargo tree` to confirm resolved dependencies match pinned SHAs

2) **Test build reproducibility**:
   - Delete Cargo.lock and run `cargo build` on different machines
   - Verify the exact same dependency versions are resolved
   - Compare checksums of built artifacts to ensure deterministic builds

3) **Verify documentation completeness**:
   - Check docs/dependency-pins.md exists and contains all git dependencies
   - Validate each entry has all required fields (SHA, date, reason, etc.)
   - Cross-reference Cargo.toml pins with documentation

4) **Test automated review process**:
   - Manually trigger the dependency review workflow
   - Verify it correctly identifies outdated dependencies
   - Confirm it creates GitHub issues with appropriate labels and details
   - Test the workflow with a intentionally outdated test dependency

5) **Security validation**:
   - Attempt to modify a git dependency to use a branch reference
   - Verify CI pipeline fails with clear error message
   - Test pre-commit hook warns about unpinned dependencies
   - Simulate a supply chain attack by creating a malicious fork and verify pinning prevents it

6) **Integration testing**:
   - Run full test suite with pinned dependencies
   - Deploy to staging environment and verify functionality
   - Monitor for any performance or compatibility issues
   - Test rollback procedure if a pinned dependency causes issues
