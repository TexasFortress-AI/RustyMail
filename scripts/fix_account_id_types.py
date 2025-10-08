#!/usr/bin/env python3
"""
Fix all remaining compilation errors from account_id refactoring.
Changes:
1. Rename account_uuid_to_db_id to account_uuid_to_email (returns String not i64)
2. Change db_account_id variable names to account_email
3. Remove calls to get_account_id_by_email
4. Update type signatures from i64 to &str where needed
"""

import re
import sys

def fix_handlers_file():
    """Fix src/dashboard/api/handlers.rs"""
    file_path = "src/dashboard/api/handlers.rs"

    with open(file_path, 'r') as f:
        content = f.read()

    # Step 1: Rename the helper function and change its return type
    content = re.sub(
        r'async fn account_uuid_to_db_id\(\n\s+account_id: &str,\n\s+state: &State,\n\) -> Result<i64, ApiError> \{',
        'async fn account_uuid_to_email(\n    account_id: &str,\n    state: &State,\n) -> Result<String, ApiError> {',
        content
    )

    # Step 2: Change the function body to return email instead of numeric ID
    # Find the function body and replace the get_account_id_by_email call
    content = re.sub(
        r'// Look up the database ID using the email address\s+let db_account_id = state\.cache_service\.get_account_id_by_email\(&email_address\)\.await\s+\.map_err\(\|e\| ApiError::InternalError\(format!\("Failed to lookup account database ID: \{\}", e\)\)\)?;\s+Ok\(db_account_id\)',
        '// Return the email address directly\n    Ok(email_address)',
        content,
        flags=re.DOTALL
    )

    # Step 3: Replace all `account_uuid_to_db_id` calls with `account_uuid_to_email`
    content = content.replace('account_uuid_to_db_id', 'account_uuid_to_email')

    # Step 4: Replace all `db_account_id` variable names with `account_email`
    content = content.replace('db_account_id', 'account_email')

    with open(file_path, 'w') as f:
        f.write(content)

    print(f"Fixed {file_path}")

def fix_sync_file():
    """Fix src/dashboard/services/sync.rs"""
    file_path = "src/dashboard/services/sync.rs"

    with open(file_path, 'r') as f:
        content = f.read()

    # Replace the get_account_id_by_email call with direct email use
    content = re.sub(
        r'let db_account_id = self\.cache_service\.get_account_id_by_email\(&account\.email_address\)\.await[^;]+;',
        'let account_email = &account.email_address;',
        content
    )

    # Replace uses of db_account_id with account_email
    content = content.replace('db_account_id', 'account_email')

    with open(file_path, 'w') as f:
        f.write(content)

    print(f"Fixed {file_path}")

def fix_email_service_file():
    """Fix src/dashboard/services/email.rs"""
    file_path = "src/dashboard/services/email.rs"

    with open(file_path, 'r') as f:
        content = f.read()

    # Replace the get_account_id_by_email call
    content = re.sub(
        r'match cache\.get_account_id_by_email\(&account\.email_address\)\.await \{[^}]+Ok\(id\) => id,[^}]+Err\(e\) => \{[^}]+continue;[^}]+\}[^}]+\}',
        '&account.email_address',
        content,
        flags=re.DOTALL
    )

    with open(file_path, 'w') as f:
        f.write(content)

    print(f"Fixed {file_path}")

if __name__ == '__main__':
    print("Fixing account_id type errors...")

    try:
        fix_handlers_file()
        fix_sync_file()
        fix_email_service_file()
        print("\nAll files fixed successfully!")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
