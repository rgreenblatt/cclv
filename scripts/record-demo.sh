#!/usr/bin/env bash
# Record a demo of cclv processing JSONL input
set -euo pipefail

INPUT_FILE="${1:-tests/fixtures/cc-session-log.jsonl}"
OUTPUT_FILE="${2:-demo.cast}"
DELAY="${DELAY:-0.1}"
MAX_LINES="${MAX_LINES:-}"

if [[ ! -f "$INPUT_FILE" ]]; then
    echo "Error: Input file not found: $INPUT_FILE" >&2
    exit 1
fi

echo "Recording demo:"
echo "  Input:  $INPUT_FILE"
echo "  Output: $OUTPUT_FILE"
echo "  Delay:  ${DELAY}s per line"
echo "  Lines:  ${MAX_LINES:-all}"
echo ""

# Build head command if MAX_LINES is set
if [[ -n "$MAX_LINES" ]]; then
    HEAD_CMD="head -n $MAX_LINES"
else
    HEAD_CMD="cat"
fi

asciinema rec --overwrite --stdin -c "
    $HEAD_CMD '$INPUT_FILE' | while IFS= read -r line; do
        echo \"\$line\"
        sleep $DELAY
    done | cargo run --release
" "$OUTPUT_FILE"

echo ""
echo "Recording saved to $OUTPUT_FILE"
echo "Convert to GIF with: agg $OUTPUT_FILE ${OUTPUT_FILE%.cast}.gif"
