"""Stage the Windows LLDB/Python layout and build its native launchers."""

import shutil
import subprocess
from pathlib import Path

from build_support import ROOT


def stage(root, llvm, debugger, paths, compiler, llvm_tools):
    native = {}

    def binary(source, destination):
        source = source.resolve(strict=True)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        native[source] = destination

    for name in llvm_tools:
        binary(llvm / "bin" / (name + ".exe"), root / "bin" / (name + ".exe"))
    private = root / "libexec/lldb"
    for name in ("lldb", "lldb-dap", "lldb-argdumper", "lldb-server"):
        binary(debugger / "bin" / (name + ".exe"), private / "bin" / (name + ".exe"))

    python_root = private / "python"
    stdlib = Path(paths["stdlib"])
    ignored = shutil.ignore_patterns(
        "site-packages",
        "__pycache__",
        "test",
        "tests",
        "idlelib",
        "tkinter",
        "turtledemo",
        "ensurepip",
        "_tkinter*",
        "*.pyc",
    )
    shutil.copytree(stdlib, python_root / "Lib", ignore=ignored)
    prefix = Path(paths["prefix"])
    shutil.copytree(prefix / "DLLs", python_root / "DLLs", ignore=ignored)
    for directory in ("Lib", "DLLs"):
        source_root = stdlib if directory == "Lib" else prefix / "DLLs"
        for destination in (python_root / directory).rglob("*"):
            if destination.suffix.lower() in (".pyd", ".dll"):
                native[
                    (source_root / destination.relative_to(python_root / directory)).resolve()
                ] = destination
    binary(Path(paths["executable"]), python_root / "python.exe")
    for name in ("python3.dll", "python" + paths["version"].replace(".", "") + ".dll"):
        binary(prefix / name, python_root / name)

    packages = [entry.parent for entry in debugger.glob("**/lldb/__init__.py")]
    if len(packages) != 1:
        raise ValueError(f"Expected one Python-enabled LLDB package in {debugger}")
    package = packages[0]
    installed = python_root / "Lib/site-packages/lldb"
    shutil.copytree(package, installed, ignore=shutil.ignore_patterns("__pycache__"))
    extensions = list(package.rglob("_lldb*.pyd"))
    if not extensions:
        raise ValueError(f"LLDB Python native module is missing in {package}")
    for source in extensions:
        native[source.resolve()] = installed / source.relative_to(package)
    argdumper = installed / "lldb-argdumper.exe"
    if not argdumper.exists():
        shutil.copy2(private / "bin/lldb-argdumper.exe", argdumper)
    # The helper is also launched directly from the Python package.
    native[argdumper.resolve()] = argdumper
    support = debugger / "share/lldb"
    if support.is_dir():
        shutil.copytree(support, private / "share/lldb")
    shutil.copytree(
        ROOT / "tooling/vscode-scoop/debugger",
        root / "share/scoop/debugger",
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    # LLDB finds its expression-parser headers relative to its own DLL.
    shutil.copytree(llvm / "lib/clang", private / "lib/clang")
    launcher = root / "bin/lldb.exe"
    subprocess.run(
        [
            str(compiler),
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-municode",
            "-static",
            str(ROOT / "scripts/windows_debugger_launcher.c"),
            "-o",
            str(launcher),
        ],
        check=True,
    )
    shutil.copy2(launcher, root / "bin/lldb-dap.exe")
    for name in ("lldb.exe", "lldb-dap.exe"):
        native[(root / "bin" / name).resolve()] = root / "bin" / name
    return native
