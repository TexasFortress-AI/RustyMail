#!/usr/bin/env python3
"""
Refactor CacheService to use email addresses instead of numeric IDs.
This script updates all method signatures and SQL queries.
"""

import re
import sys

def refactor_cache_service(input_file, output_file):
    with open(input_file, 'r') as f:
        content = f.read()

    # Step 1: Remove get_account_id_by_email method entirely (lines 205-216)
    content = re.sub(
        r'    /// Get account\'s numeric ID by email address\n'
        r'    pub async fn get_account_id_by_email\(&self, email: &str\) -> Result<i64, CacheError> \{[\s\S]*?\n    \}\n\n',
        '',
        content
    )

    # Step 2: Change all method signatures from `account_id: i64` to `account_id: &str`
    content = content.replace('account_id: i64', 'account_id: &str')

    # Step 3: Update all folder cache keys to use email addresses directly
    # Old: format!("{}:{}", account_id, name)
    # New: format!("{}:{}", account_id, name)  (stays the same, but now account_id is &str)

    # Step 4: Update SQL queries - they already bind account_id, just need to ensure TEXT type
    # The database migration already changed the column type, so no SQL changes needed

    with open(output_file, 'w') as f:
        f.write(content)

    print(f"Refactored {input_file} -> {output_file}")
    print("Changes made:")
    print("  - Removed get_account_id_by_email method")
    print("  - Changed all 'account_id: i64' to 'account_id: &str'")

if __name__ == '__main__':
    input_file = '/Users/au/src/RustyMail/src/dashboard/services/cache.rs'
    output_file = input_file

    # Backup first
    import shutil
    backup_file = input_file + '.backup'
    shutil.copy(input_file, backup_file)
    print(f"Created backup: {backup_file}")

    refactor_cache_service(input_file, output_file)
    print("Done!")
