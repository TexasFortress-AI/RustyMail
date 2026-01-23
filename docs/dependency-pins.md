# Pinned Git Dependencies

This document tracks all git dependencies that are pinned to specific commit SHAs for supply chain security.

## Why Pin Dependencies?

Git dependencies using `branch` or `tag` references can change at any time if the upstream repository is modified. Pinning to specific commit SHAs ensures:

1. **Reproducible builds** - The same code is always fetched
2. **Supply chain security** - Prevents malicious code injection via upstream changes
3. **Audit trail** - Clear record of exactly what code is being used

## Pinned Dependencies

| Crate | Repository | Pinned SHA | Pin Date | Reviewed By |
|-------|-----------|------------|----------|-------------|
| rmcp | https://github.com/modelcontextprotocol/rust-sdk.git | e623f2acabd53e51a978d160f955c315bc16c220 | 2026-01-23 | Claude Code |
| rmcp-macros | https://github.com/modelcontextprotocol/rust-sdk.git | e623f2acabd53e51a978d160f955c315bc16c220 | 2026-01-23 | Claude Code |

## Update Procedure

When updating a pinned dependency:

1. **Review upstream changes** - Check the commit history since the pinned version
2. **Check for security advisories** - Look for any CVEs or security issues
3. **Test in isolation** - Update in a branch and run full test suite
4. **Update this document** - Record the new SHA and review date
5. **Update Cargo.toml** - Change the `rev` value to the new SHA

## Checking for Updates

To check if upstream has new commits:

```bash
# Get latest commit on main branch
git ls-remote https://github.com/modelcontextprotocol/rust-sdk.git refs/heads/main

# Compare with pinned SHA in Cargo.toml
grep "rmcp.*rev" Cargo.toml
```

## Review Schedule

Pinned dependencies should be reviewed monthly for:
- Security patches
- Bug fixes
- New features needed by RustyMail

Last review: 2026-01-23
Next scheduled review: 2026-02-23
