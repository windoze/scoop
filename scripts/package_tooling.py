"""Build the installable VS Code extension shipped with the toolchain."""

import json
import subprocess

from build_support import ROOT


def build(work_dir, environment):
    tooling = ROOT / "tooling"
    manifest = json.loads((tooling / "vscode-scoop/package.json").read_text())
    output = work_dir / "tooling" / f"{manifest['name']}-{manifest['version']}.vsix"
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["npm", "ci"], cwd=tooling, env=environment, check=True)
    # The package script's prepublish hook builds both the parser WASM and extension.
    subprocess.run(
        [
            "npm",
            "run",
            "package",
            "--workspace",
            "scoop-language-support",
            "--",
            "--out",
            str(output),
        ],
        cwd=tooling,
        env=environment,
        check=True,
    )
    return output
