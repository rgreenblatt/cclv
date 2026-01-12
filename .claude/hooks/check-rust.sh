#!/usr/bin/env bash
# Post-edit hook: run cargo clippy on Rust file edits
# Rejects edits that introduce errors or warnings
# Formats the file with rustfmt after clippy passes

# Read JSON input from stdin
input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only check .rs files
if [[ ! "$file_path" == *.rs ]]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR/crates" || exit 0

# Run cargo clippy (catches both errors and warnings)
# --all-targets includes tests, benches, examples
output=$(cargo clippy --all-targets -- -D warnings 2>&1)
result=$?

if [[ $result -ne 0 ]]; then
  echo "cargo clippy failed:" >&2
  echo "$output" >&2
  exit 2  # Exit 2 = blocking error, rejects the edit
fi

# Format the edited file with rustfmt (through treefmt-nix)
nix fmt "$file_path" 2>&1

exit 0
