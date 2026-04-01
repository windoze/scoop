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

    u=$(uuidgen)
    uuid=$(echo "$u" | tr '[:upper:]' '[:lower:]')
    title="Scoop-Codex-$(date +%Y-%m-%d-%H-%M-%S)"

    echo "Running codex with PROMPT.md..."
    cwcli --token "$CW_TOKEN" send -w "1d317780-ef53-48f1-89de-3e94c69c24a6" "$(cat "$PROMPT_FILE")"
    "$REPO_DIR/notification.sh"
done
