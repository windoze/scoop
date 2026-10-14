"""Select real GNU linkers through the compiler's ordinary -B option."""

import shlex
import shutil
import subprocess
import sys
from pathlib import Path

mode, directory, compiler = sys.argv[1:]
root = Path(directory)
alias = root / "ld"
if mode == "setup":
    root.mkdir()
    compiler = Path(compiler).resolve()
    selected = subprocess.check_output([compiler, "-print-prog-name=ld"], text=True).strip()
    linker = Path(shutil.which(selected)).resolve()
    for name in ["first", "second"]:
        wrapper = root / name
        wrapper.write_text(
            f"#!/bin/sh\necho {name} >> {shlex.quote(str(root / 'calls'))}\n"
            f'exec {shlex.quote(str(linker))} "$@"\n'
        )
        wrapper.chmod(0o755)
    driver = root / "gcc"
    driver.write_text(
        f'#!/bin/sh\nexec {shlex.quote(str(compiler))} -B {shlex.quote(str(root) + "/")} "$@"\n'
    )
    driver.chmod(0o755)
    alias.symlink_to(root / "first")
elif mode == "retarget":
    alias.unlink()
    alias.symlink_to(root / "second")
else:
    raise ValueError(mode)
