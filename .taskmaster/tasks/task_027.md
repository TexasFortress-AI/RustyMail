# Task ID: 27

**Title:** Fix path traversal vulnerability in attachment_storage.rs

**Status:** done

**Dependencies:** 32 ✓

**Priority:** medium

**Description:** Implement secure path canonicalization and containment checks in attachment_storage.rs to prevent directory traversal attacks via malicious file paths or symlinks.

**Details:**

Fix the path traversal vulnerability in attachment_storage.rs by implementing comprehensive path security measures:

1) **Add path canonicalization**:
   - Import std::fs::canonicalize() to resolve all symbolic links and relative path components
   - Before any file operation, canonicalize both the requested path and the storage root directory
   - Handle canonicalization errors gracefully (non-existent paths, permission issues)

2) **Implement strict containment validation**:
   - Create a validate_path_containment() function that:
     - Canonicalizes the requested file path
     - Canonicalizes the attachments storage root directory
     - Uses path.starts_with() to ensure the resolved path is within the storage root
     - Returns Result<PathBuf, SecurityError> with the safe canonicalized path or error
   
3) **Update all file operations**:
   - Modify save_attachment(), get_attachment(), delete_attachment() to use validate_path_containment()
   - Replace current basic path component checks with the new validation
   - Ensure all Path/PathBuf constructions go through validation before use

4) **Handle edge cases**:
   - Reject null bytes in filenames
   - Validate against Windows reserved names (CON, PRN, AUX, etc.) if cross-platform
   - Handle Unicode normalization attacks (different representations of same character)
   - Prevent TOCTOU attacks by using the validated canonical path for operations

5) **Example implementation**:
   ```rust
   use std::path::{Path, PathBuf};
   use std::fs;
   
   #[derive(Debug, thiserror::Error)]
   enum PathSecurityError {
       #[error("Path traversal attempt detected")]
       PathTraversal,
       #[error("Invalid path: {0}")]
       InvalidPath(String),
       #[error("Canonicalization failed: {0}")]
       CanonicalizationError(#[from] std::io::Error),
   }
   
   fn validate_path_containment(
       storage_root: &Path,
       requested_path: &Path
   ) -> Result<PathBuf, PathSecurityError> {
       // Canonicalize the storage root
       let canonical_root = fs::canonicalize(storage_root)?;
       
       // Construct full path and canonicalize
       let full_path = storage_root.join(requested_path);
       let canonical_path = fs::canonicalize(&full_path)
           .or_else(|_| {
               // If file doesn't exist, canonicalize parent and append filename
               let parent = full_path.parent()
                   .ok_or_else(|| PathSecurityError::InvalidPath("No parent directory".into()))?;
               let filename = full_path.file_name()
                   .ok_or_else(|| PathSecurityError::InvalidPath("No filename".into()))?;
               
               let canonical_parent = fs::canonicalize(parent)?;
               Ok(canonical_parent.join(filename))
           })?;
       
       // Verify the canonical path is within the storage root
       if !canonical_path.starts_with(&canonical_root) {
           return Err(PathSecurityError::PathTraversal);
       }
       
       Ok(canonical_path)
   }
   ```

6) **Add security logging**:
   - Log all path traversal attempts with source IP/user info
   - Include the malicious path in logs for security monitoring
   - Consider rate limiting after multiple traversal attempts

**Test Strategy:**

Verify the path traversal fix with comprehensive security testing:

1) **Unit tests for path validation**:
   - Test basic traversal attempts: "../../../etc/passwd", "..\\..\\windows\\system32"
   - Test encoded traversals: "%2e%2e%2f", "..%252f", "%c0%ae%c0%ae/"
   - Test symlink traversal: create symlink pointing outside storage, verify rejection
   - Test absolute paths: "/etc/passwd", "C:\\Windows\\System32"
   - Test null bytes: "file.txt\x00.pdf"
   - Test Unicode tricks: "ﬁle.txt" (ligature), different normalization forms

2) **Integration tests**:
   - Create test storage directory with known structure
   - Attempt to save files with malicious paths, verify all are rejected
   - Test legitimate nested paths work correctly: "user123/2024/invoice.pdf"
   - Verify error messages don't leak system paths

3) **Edge case testing**:
   - Test very long paths (near filesystem limits)
   - Test special filenames: ".", "..", "~", "$file"
   - Test Windows reserved names: "CON", "PRN", "AUX", "NUL"
   - Test case sensitivity issues on case-insensitive filesystems

4) **TOCTOU race condition test**:
   - Create a legitimate file
   - In parallel thread, try to replace it with symlink during validation
   - Verify the operation uses the validated canonical path

5) **Performance testing**:
   - Measure overhead of canonicalization on typical operations
   - Test with deeply nested directory structures
   - Ensure no significant performance regression

6) **Security audit checklist**:
   - Verify all file operations use validate_path_containment()
   - Check no direct Path construction from user input
   - Confirm error messages don't reveal system structure
   - Review logs for attempted traversals during testing
