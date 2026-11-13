"""Prepare LLVM source inputs and stage the tools needed by installed Scoop."""

import filecmp
import json
import os
import shutil
import tarfile
import tempfile
from pathlib import Path

from build_llvm import VERSION
from build_support import ROOT, metadata
from download_support import download_directory, obtain

TOOLS = (
    "clang",
    "clang++",
    "clang-cl",
    "lld",
    "lld-link",
    "ld.lld",
    "llvm-ar",
    "llvm-ranlib",
    "llvm-readobj",
    "llvm-nm",
    "llvm-objcopy",
    "llvm-strip",
    "llvm-symbolizer",
    "llvm-profdata",
    "dsymutil",
)
WINDOWS_TOOLS = ("llvm-rc", "llvm-windres", "llvm-cvtres")


def share_file(source, destination, *, windows):
    """Create an alias to an already staged file, within this component only."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() and source.samefile(destination):
        return
    destination.unlink(missing_ok=True)
    if windows:
        os.link(source, destination)
    else:
        destination.symlink_to(os.path.relpath(source, destination.parent))


def stage(root, prefix, host_system):
    """Select runtime tools and builtin headers, leaving the development SDK intact."""
    windows = host_system == "Windows"
    suffix = ".exe" if windows else ""
    private = root / "libexec/llvm"
    (private / "bin").mkdir(parents=True)
    native = {}
    for name in (*TOOLS, *(WINDOWS_TOOLS if windows else ())):
        source = (prefix / "bin" / (name + suffix)).resolve(strict=True)
        destination = private / "bin" / (name + suffix)
        if source in native and not windows:
            share_file(native[source], destination, windows=False)
        else:
            shutil.copy2(source, destination)
            native[source] = destination
    resources = prefix / "lib/clang" / VERSION.split(".")[0]
    shutil.copytree(resources / "include", private / "lib/clang" / resources.name / "include")
    return native


def stage_unwind_headers(root, source):
    """Share LLVM's public unwind declarations between target link probes."""
    destination = root / "libexec/scoop/include/unwind"
    destination.mkdir(parents=True)
    for path in (source / "libunwind/include").iterdir():
        if path.is_file() and path.suffix == ".h":
            shutil.copy2(path, destination / path.name)


def share_windows_aliases(root, public_tools):
    """Share known executable/resource aliases after staging all loader dependencies."""
    llvm = root / "libexec/llvm/bin"
    groups = [
        [llvm / (name + ".exe") for name in names]
        for names in (
            ("clang", "clang++", "clang-cl"),
            ("lld", "lld-link", "ld.lld"),
            ("llvm-ar", "llvm-ranlib"),
            ("llvm-objcopy", "llvm-strip"),
        )
    ]
    groups += [
        [llvm / (name + ".exe"), root / "bin" / ("scoop-" + name + ".exe")] for name in public_tools
    ]
    private = root / "libexec/lldb"
    groups += [
        [root / "bin/scoop-lldb.exe", root / "bin/scoop-lldb-dap.exe"],
        [private / "bin/lldb-argdumper.exe", *private.glob("python/**/lldb-argdumper.exe")],
        [*private.rglob("liblldb.dll"), *private.glob("python/**/_lldb*.pyd")],
    ]
    for paths in groups:
        remaining = list(paths)
        while remaining:
            source = remaining.pop(0)
            for destination in remaining[:]:
                if filecmp.cmp(source, destination, shallow=False):
                    share_file(source, destination, windows=True)
                    remaining.remove(destination)


def prepare(work_dir, source, archive, download_dir=None):
    if source is not None:
        source = source.resolve(strict=True)
    name = f"llvm-project-{VERSION}.src"
    vendor = ROOT / "sysroot/vendor/llvm-project" / VERSION
    inputs = work_dir / "inputs"
    inputs.mkdir(parents=True, exist_ok=True)
    default_archive = archive is None
    if default_archive:
        archive = obtain(
            download_directory(download_dir),
            "llvm-project",
            VERSION,
            f"{name}.tar.xz",
            f"https://github.com/llvm/llvm-project/releases/download/llvmorg-{VERSION}/{name}.tar.xz",
            local=vendor / f"{name}.tar.xz",
        )
    archive = archive.resolve(strict=True)
    if source is None:
        if default_archive and (vendor / name).is_dir():
            return (vendor / name).resolve(), archive
        source = inputs / name
        record = inputs / "llvm-source.json"
        current = metadata(archive)
        if not (source.is_dir() and record.is_file() and json.loads(record.read_text()) == current):
            print(f"Extracting {archive}", flush=True)
            with tempfile.TemporaryDirectory(prefix="llvm-source-", dir=inputs) as temporary:
                with tarfile.open(archive) as package:
                    package.extractall(temporary, filter="data")
                extracted = Path(temporary) / name
                if not extracted.is_dir():
                    raise ValueError(f"LLVM archive does not contain {name}")
                if source.exists():
                    shutil.rmtree(source)
                extracted.replace(source)
            record.write_text(json.dumps(current) + "\n")
    return source.resolve(strict=True), archive
