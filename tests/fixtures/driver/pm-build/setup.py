"""Prepare a real local Git dependency for the workspace CLI fixture."""

import json
import subprocess
import sys
from pathlib import Path

work, paired_pm = map(Path, sys.argv[1:])
repository = work / "repository"


def git(*arguments):
    return subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


git("init", "--quiet")
git("add", ".")
git(
    "-c",
    "user.name=Scoop tests",
    "-c",
    "user.email=tests@example.invalid",
    "commit",
    "--allow-empty",
    "-qm",
    "Initial package",
)
commit = git("rev-parse", "HEAD")
manifest = work / "project/Cone.toml"
manifest.write_text(
    manifest.read_text().replace("GIT_URL", repository.as_uri()).replace("GIT_REV", commit)
)
(work / "pm-calls").write_text("")
wrapper = work / "counting-pm"
wrapper.write_text(
    "#!/usr/bin/env python3\nimport os, sys\n"
    f"with open({json.dumps(str(work / 'pm-calls'))}, 'a') as calls: calls.write('prepare\\n')\n"
    f"os.execv({json.dumps(str(paired_pm))}, [{json.dumps(str(paired_pm))}, *sys.argv[1:]])\n"
)
wrapper.chmod(0o755)
