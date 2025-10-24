#!/usr/bin/env python3
"""
Script to add MPL 2.0 license headers to source files.
Safe to run multiple times - will not add duplicates.
"""

import os
import sys
from pathlib import Path

LICENSE_HEADER = """// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

"""

def has_license_header(file_path):
    """Check if file already has the license header."""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            first_lines = ''.join(f.readlines()[:5])
            return 'Mozilla Public License' in first_lines
    except Exception as e:
        print(f"  ⚠️  Error reading {file_path}: {e}")
        return True  # Skip on error

def add_license_to_file(file_path):
    """Add license header to a file."""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()

        # Check if file starts with shebang
        if content.startswith('#!'):
            # Preserve shebang
            lines = content.split('\n', 1)
            new_content = lines[0] + '\n\n' + LICENSE_HEADER + (lines[1] if len(lines) > 1 else '')
        else:
            # Add license at the top
            new_content = LICENSE_HEADER + content

        # Write back
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(new_content)

        return True
    except Exception as e:
        print(f"  ❌ Error processing {file_path}: {e}")
        return False

def find_files(directory, extensions, exclude_dirs=None):
    """Find all files with given extensions, excluding certain directories."""
    if exclude_dirs is None:
        exclude_dirs = {'node_modules', 'dist', '.vite', 'target', '.git'}

    files = []
    for root, dirs, filenames in os.walk(directory):
        # Remove excluded directories from search
        dirs[:] = [d for d in dirs if d not in exclude_dirs]

        for filename in filenames:
            if any(filename.endswith(ext) for ext in extensions):
                files.append(os.path.join(root, filename))

    return files

def main():
    """Main function to process all source files."""
    added = 0
    skipped = 0
    total = 0

    print("=" * 50)
    print("Adding MPL 2.0 License Headers")
    print("=" * 50)
    print()

    # Process Rust files in src/
    print("📁 Processing Rust files in src/...")
    rust_src_files = find_files('src', ['.rs'])
    for file_path in sorted(rust_src_files):
        total += 1
        if has_license_header(file_path):
            print(f"  ⏭  Skipped (already has license): {file_path}")
            skipped += 1
        else:
            if add_license_to_file(file_path):
                print(f"  ✅ Added license to: {file_path}")
                added += 1

    # Process Rust files in tests/
    print()
    print("📁 Processing Rust files in tests/...")
    rust_test_files = find_files('tests', ['.rs'])
    for file_path in sorted(rust_test_files):
        total += 1
        if has_license_header(file_path):
            print(f"  ⏭  Skipped (already has license): {file_path}")
            skipped += 1
        else:
            if add_license_to_file(file_path):
                print(f"  ✅ Added license to: {file_path}")
                added += 1

    # Process frontend files
    if os.path.exists('frontend'):
        print()
        print("📁 Processing TypeScript/JavaScript files in frontend/...")
        frontend_extensions = ['.ts', '.tsx', '.js', '.jsx', '.vue']
        frontend_files = find_files('frontend', frontend_extensions)
        for file_path in sorted(frontend_files):
            total += 1
            if has_license_header(file_path):
                print(f"  ⏭  Skipped (already has license): {file_path}")
                skipped += 1
            else:
                if add_license_to_file(file_path):
                    print(f"  ✅ Added license to: {file_path}")
                    added += 1

    # Summary
    print()
    print("=" * 50)
    print("Summary")
    print("=" * 50)
    print(f"Total files processed: {total}")
    print(f"License headers added: {added}")
    print(f"Files skipped (already licensed): {skipped}")
    print()

    if added > 0:
        print(f"✅ Successfully added {added} license headers!")
    else:
        print("ℹ️  No new license headers needed - all files already have them.")

    return 0 if total > 0 else 1

if __name__ == '__main__':
    sys.exit(main())
