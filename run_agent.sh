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

    uuid=$(uuidgen)
    uuid="${uuid,,}"    # Convert to lowercase
    title="Scoop-Codex-$(date +%Y-%m-%d-%H-%M-%S)-$uuid"

    echo "Running codex with PROMPT.md..."
    cwcli --token "$CW_TOKEN" create --project-id "$uuid" --title "$title"
    cwcli --token "$CW_TOKEN" send -w "$uuid" "$(cat "$PROMPT_FILE")"
    "$REPO_DIR/notification.sh"
done
