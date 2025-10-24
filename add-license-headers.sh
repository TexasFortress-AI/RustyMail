#!/bin/bash
# Script to add MPL 2.0 license headers to source files
# Safe to run multiple times - will not add duplicates

set -e

# Copyright notice to add
read -r -d '' LICENSE_HEADER << 'EOF' || true
// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

EOF

# Counters
ADDED=0
SKIPPED=0
TOTAL=0

# Function to add license header to a file
add_license_header() {
    local file="$1"

    # Skip if file doesn't exist or is empty
    if [ ! -f "$file" ] || [ ! -s "$file" ]; then
        return
    fi

    # Check if license header already exists
    if head -n 5 "$file" | grep -q "Mozilla Public License"; then
        echo "  ⏭  Skipped (already has license): $file"
        ((SKIPPED++))
        return
    fi

    # Create temporary file
    local tmpfile=$(mktemp)

    # Check if file starts with shebang
    if head -n 1 "$file" | grep -q '^#!'; then
        # Preserve shebang, add license after it
        head -n 1 "$file" > "$tmpfile"
        echo "" >> "$tmpfile"
        echo "$LICENSE_HEADER" >> "$tmpfile"
        tail -n +2 "$file" >> "$tmpfile"
    else
        # No shebang, add license at the top
        echo "$LICENSE_HEADER" > "$tmpfile"
        cat "$file" >> "$tmpfile"
    fi

    # Replace original file with new content
    mv "$tmpfile" "$file"

    echo "  ✅ Added license to: $file"
    ((ADDED++))
}

echo "=========================================="
echo "Adding MPL 2.0 License Headers"
echo "=========================================="
echo ""

# Process Rust files in src/
echo "📁 Processing Rust files in src/..."
while IFS= read -r -d '' file; do
    ((TOTAL++))
    add_license_header "$file"
done < <(find src -type f -name "*.rs" -print0 2>/dev/null)

# Process Rust files in tests/
echo ""
echo "📁 Processing Rust files in tests/..."
while IFS= read -r -d '' file; do
    ((TOTAL++))
    add_license_header "$file"
done < <(find tests -type f -name "*.rs" -print0 2>/dev/null)

# Process TypeScript/JavaScript files in frontend/
if [ -d "frontend" ]; then
    echo ""
    echo "📁 Processing TypeScript/JavaScript files in frontend/..."

    # Process .ts files
    while IFS= read -r -d '' file; do
        ((TOTAL++))
        add_license_header "$file"
    done < <(find frontend -type f -name "*.ts" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.vite/*" -print0 2>/dev/null)

    # Process .tsx files
    while IFS= read -r -d '' file; do
        ((TOTAL++))
        add_license_header "$file"
    done < <(find frontend -type f -name "*.tsx" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.vite/*" -print0 2>/dev/null)

    # Process .js files
    while IFS= read -r -d '' file; do
        ((TOTAL++))
        add_license_header "$file"
    done < <(find frontend -type f -name "*.js" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.vite/*" -print0 2>/dev/null)

    # Process .jsx files
    while IFS= read -r -d '' file; do
        ((TOTAL++))
        add_license_header "$file"
    done < <(find frontend -type f -name "*.jsx" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.vite/*" -print0 2>/dev/null)

    # Process .vue files
    while IFS= read -r -d '' file; do
        ((TOTAL++))
        add_license_header "$file"
    done < <(find frontend -type f -name "*.vue" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.vite/*" -print0 2>/dev/null)
fi

echo ""
echo "=========================================="
echo "Summary"
echo "=========================================="
echo "Total files processed: $TOTAL"
echo "License headers added: $ADDED"
echo "Files skipped (already licensed): $SKIPPED"
echo ""

if [ $ADDED -gt 0 ]; then
    echo "✅ Successfully added $ADDED license headers!"
else
    echo "ℹ️  No new license headers needed - all files already have them."
fi
