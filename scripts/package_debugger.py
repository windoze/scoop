"""Stage LLDB, its matching Python runtime and native loader inputs."""

import json
import os
import shutil
import subprocess
from pathlib import Path

import package_debugger_windows
from build_support import ROOT

LLVM_TOOLS = (
    "dsymutil",
    "llvm-objcopy",
    "llvm-strip",
    "llvm-symbolizer",
    "llvm-nm",
    "llvm-readobj",
)


def python_paths(executable):
    script = """
import json, sys, sysconfig
print(json.dumps({
  'version': f'{sys.version_info.major}.{sys.version_info.minor}',
  'executable': sys._base_executable,
  'stdlib': sysconfig.get_path('stdlib'),
  'prefix': sys.base_prefix,
}))
"""
    return json.loads(subprocess.check_output([executable, "-c", script], text=True))


def python_license(paths, explicit=None):
    if explicit is not None:
        return explicit.resolve(strict=True)
    candidates = [Path(paths[key]) / "LICENSE.txt" for key in ("stdlib", "prefix")]
    candidates.append(Path("/usr/share/doc") / ("python" + paths["version"]) / "copyright")
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise ValueError("Cannot locate the Python license; pass --python-license")


def launcher(name, version, server):
    command = ' "$@"'
    if name == "lldb":
        command = " -o 'command script import scoop_lldb'" + command
    return f'''#!/bin/sh
scoop_debug_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd) || exit 1
export PYTHONHOME="$scoop_debug_root/libexec/lldb/python"
export PYTHONPATH="$PYTHONHOME/lib/python{version}/site-packages:$scoop_debug_root/share/scoop/debugger"
export PYTHONNOUSERSITE=1
export LLDB_DEBUGSERVER_PATH="$scoop_debug_root/libexec/lldb/bin/{server}"
exec "$scoop_debug_root/libexec/lldb/bin/{name}"{command}
'''


def stage(root, llvm, debugger, python, host, license_file=None, *, windows_compiler=None):
    paths = python_paths(python)
    version = paths["version"]
    license_file = python_license(paths, license_file)
    if "windows" in host:
        return (
            package_debugger_windows.stage(
                root, llvm, debugger, paths, windows_compiler, LLVM_TOOLS
            ),
            version,
            license_file,
        )
    private = root / "libexec/llvm/lib"
    private.mkdir(parents=True, exist_ok=True)
    native = {}

    def binary(source, destination):
        source = source.resolve(strict=True)
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source in native:
            destination.symlink_to(os.path.relpath(native[source], destination.parent))
        else:
            shutil.copy2(source, destination)
            native[source] = destination

    for name in LLVM_TOOLS:
        binary(llvm / "bin" / name, root / "bin" / ("scoop-" + name))
    server = "debugserver" if host.endswith("darwin") else "lldb-server"
    for name in ("lldb", "lldb-dap", "lldb-argdumper", server):
        binary(debugger / "bin" / name, root / "libexec/lldb/bin" / name)
    stdlib = Path(paths["stdlib"])
    python_root = root / "libexec/lldb/python"
    destination = python_root / "lib" / ("python" + version)
    shutil.copytree(
        stdlib,
        destination,
        ignore=shutil.ignore_patterns(
            "site-packages",
            "dist-packages",
            "__pycache__",
            "test",
            "tests",
            "idlelib",
            "tkinter",
            "turtledemo",
            "ensurepip",
            "config-*",
            "_tkinter*",
            "*.pyc",
        ),
    )
    for source in stdlib.rglob("*.so"):
        copied = destination / source.relative_to(stdlib)
        if copied.is_file():
            native[source.resolve()] = copied
    binary(Path(paths["executable"]), python_root / "bin" / ("python" + version))
    (python_root / "bin/python3").symlink_to("python" + version)
    packages = [entry.parent for entry in debugger.glob("**/lldb/__init__.py")]
    if len(packages) != 1:
        raise ValueError(f"Expected one Python-enabled LLDB package in {debugger}")
    package = packages[0]
    installed = destination / "site-packages/lldb"
    shutil.copytree(
        package, installed, ignore=shutil.ignore_patterns("__pycache__", "_lldb*", "lldb-argdumper")
    )
    (installed / "lldb-argdumper").symlink_to(
        os.path.relpath(root / "libexec/lldb/bin/lldb-argdumper", installed)
    )
    extensions = list(package.rglob("_lldb*.so"))
    if not extensions:
        raise ValueError(f"LLDB Python native module is missing in {package}")
    for extension in extensions:
        source = extension.resolve(strict=True)
        output = installed / extension.relative_to(package)
        if source.name.startswith("liblldb"):
            if source not in native:
                binary(source, private / source.name)
            output.symlink_to(os.path.relpath(native[source], output.parent))
        else:
            binary(source, output)
    resources = llvm / "lib/clang"
    shutil.copytree(resources, private / "clang")
    support = debugger / "share/lldb"
    if support.is_dir():
        shutil.copytree(support, root / "libexec/lldb/share/lldb")
    formatters = root / "share/scoop/debugger"
    shutil.copytree(
        ROOT / "tooling/vscode-scoop/debugger",
        formatters,
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    for name in ("lldb", "lldb-dap"):
        output = root / "bin" / ("scoop-" + name)
        output.write_text(launcher(name, version, server))
        output.chmod(0o755)
    return native, version, license_file
