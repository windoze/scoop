"""Compile the opposite Darwin architecture for the native-input diagnostic."""

import subprocess
import sys

target, *command = sys.argv[1:]
opposite = {
    "aarch64-apple-darwin": "x86_64-apple-darwin",
    "x86_64-apple-darwin": "aarch64-apple-darwin",
}[target]
raise SystemExit(subprocess.call([*command, "-target", opposite]))
