#!/bin/bash

# Script to sync Cursor rules to Cline rules
# Copies .cursorrules and .cursor/rules/*.mdc into a single .clinerules file

# Define source and target paths
CURSOR_ROOT_RULES=".cursorrules"
CURSOR_RULES_DIR=".cursor/rules"
CLINE_RULES_FILE=".clinerules"

# Check if running in a project directory with Cursor rules
if [ ! -f "$CURSOR_ROOT_RULES" ] && [ ! -d "$CURSOR_RULES_DIR" ]; then
  echo "Error: No Cursor rules found. Ensure .cursorrules or .cursor/rules/ exists in the current directory."
  exit 1
fi

# Start fresh by removing the existing .clinerules file (if any)
if [ -f "$CLINE_RULES_FILE" ]; then
  rm "$CLINE_RULES_FILE"
fi

# Add a header to the new .clinerules file
echo "# Cline Rules (Auto-Generated from Cursor Rules)" > "$CLINE_RULES_FILE"
echo "# Generated on $(date)" >> "$CLINE_RULES_FILE"
echo "" >> "$CLINE_RULES_FILE"

# Function to append a file's content with a section header
append_file_content() {
  local source_file="$1"
  local section_name="$2"
  if [ -f "$source_file" ]; then
    echo "## $section_name" >> "$CLINE_RULES_FILE"
    cat "$source_file" >> "$CLINE_RULES_FILE"
    echo "" >> "$CLINE_RULES_FILE"  # Add a blank line for readability
  fi
}

# Append .cursorrules if it exists
append_file_content "$CURSOR_ROOT_RULES" "Root Cursor Rules (.cursorrules)"

# Append all .mdc files from .cursor/rules/ if the directory exists
if [ -d "$CURSOR_RULES_DIR" ]; then
  # Check if there are any .mdc files
  if ls "$CURSOR_RULES_DIR"/*.mdc >/dev/null 2>&1; then
    echo "## Rules from .cursor/rules/*.mdc" >> "$CLINE_RULES_FILE"
    for mdc_file in "$CURSOR_RULES_DIR"/*.mdc; do
      # Extract the filename without the path for the section header
      filename=$(basename "$mdc_file")
      echo "" >> "$CLINE_RULES_FILE"
      echo "### $filename" >> "$CLINE_RULES_FILE"
      # Handle .mdc-specific fields (e.g., description:, globs:) if present
      while IFS= read -r line; do
        if [[ "$line" =~ ^description: ]]; then
          echo "Description: ${line#description: }" >> "$CLINE_RULES_FILE"
        elif [[ "$line" =~ ^globs: ]]; then
          echo "Applies to: ${line#globs: }" >> "$CLINE_RULES_FILE"
        elif [[ "$line" =~ ^alwaysApply: ]]; then
          echo "Always Apply: ${line#alwaysApply: }" >> "$CLINE_RULES_FILE"
        else
          echo "$line" >> "$CLINE_RULES_FILE"
        fi
      done < "$mdc_file"
    done
  else
    echo "No .mdc files found in $CURSOR_RULES_DIR." >> "$CLINE_RULES_FILE"
  fi
else
  echo "# Note: No .cursor/rules/ directory found." >> "$CLINE_RULES_FILE"
fi

# Finalize
echo "" >> "$CLINE_RULES_FILE"
echo "# End of auto-generated rules" >> "$CLINE_RULES_FILE"

echo "Sync complete! .clinerules has been generated from Cursor rules."

