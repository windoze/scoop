"""The three shipped commands identify the same build without loading a sysroot."""

import subprocess
import sys

versions = []
for name, path in zip(("scoop", "scoopc", "scoop-link"), sys.argv[1:], strict=True):
    result = subprocess.run([path, "--version"], check=True, capture_output=True, text=True)
    assert not result.stderr, result.stderr
    tool, version = result.stdout.strip().split(" ", 1)
    assert tool == name, result.stdout
    assert " (" in version and version.endswith(")"), version
    versions.append(version)
assert len(set(versions)) == 1, versions
print("matching versions")
