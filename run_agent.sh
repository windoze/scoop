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
    # cwcli --token "$CW_TOKEN" send -w dab07a9c-c526-4edd-8a70-e14e3252d123 "$(cat "$PROMPT_FILE")"
    codex  exec --dangerously-bypass-approvals-and-sandbox "$(cat "$PROMPT_FILE")"
    # claude --print --dangerously-skip-permissions "$(cat "$PROMPT_FILE")" 2>&1 | tee /tmp/scoop-cc.log
    "$REPO_DIR/notification.sh"
    git push
done
