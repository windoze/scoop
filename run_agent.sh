#!/bin/bash
set -e

PROMPT_FILE="$(dirname "$0")/PROMPT.md"
REPO_DIR="$(dirname "$0")"

while true; do
    # Check if v0.1.0 tag exists
    if git -C "$REPO_DIR" tag | grep -q '^v0\.1\.0$'; then
        echo "Tag v0.1.0 found. Done."
        break
    fi

    echo "Running codex with PROMPT.md..."
    codex  exec --dangerously-bypass-approvals-and-sandbox "$(cat "$PROMPT_FILE")" >> ~/tmp/scoop_1_codex_output.txt
    "$REPO_DIR/notification.sh"
done
