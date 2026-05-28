#!/bin/bash
set -e
set -o pipefail

PROMPT_FILE="$(dirname "$0")/PROMPT.md"
REPO_DIR="$(dirname "$0")"

# AGENT_CLI 决定使用哪个 agent CLI: opencode | claude | codex | copilot (默认) | cwcli
AGENT_CLI="${AGENT_CLI:-copilot}"

run_agent_cli() {
    local prompt
    prompt="$(cat "$PROMPT_FILE")"
    case "$AGENT_CLI" in
        opencode)
            opencode run --dangerously-skip-permissions "$prompt"
            ;;
        claude)
            claude --print --effort max --dangerously-skip-permissions "$prompt" 2>&1 | tee /tmp/scoop-cc.log
            ;;
        codex)
            codex exec --dangerously-bypass-approvals-and-sandbox "$prompt"
            ;;
        copilot)
            copilot --effort xhigh --yolo -p "$prompt"
            ;;
        cwcli)
            cwcli --token "$CW_TOKEN" send -w dab07a9c-c526-4edd-8a70-e14e3252d123 "$prompt"
            ;;
        *)
            echo "Unknown AGENT_CLI: $AGENT_CLI" >&2
            return 2
            ;;
    esac
}

while true; do
    # Check if v0.1.0 tag exists
    if git -C "$REPO_DIR" tag | grep -q '^v0\.1\.0$'; then
        echo "Tag v0.1.0 found. Done."
        break
    fi

    echo "Running $AGENT_CLI with PROMPT.md..."
    if ! run_agent_cli; then
        echo "Agent CLI '$AGENT_CLI' failed. Waiting 60s before retry..." >&2
        sleep 600
        continue
    fi
    "$REPO_DIR/notification.sh"
    git push
done
