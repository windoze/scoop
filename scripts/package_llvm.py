"""Prepare the fixed upstream LLVM source input for toolchain packaging."""

import json
import shutil
import tarfile
import tempfile
import urllib.request
from pathlib import Path

from build_llvm import VERSION
from build_support import ROOT, metadata


def prepare(work_dir, source, archive):
    if source is not None:
        source = source.resolve(strict=True)
    name = f"llvm-project-{VERSION}.src"
    vendor = ROOT / "sysroot/vendor/llvm-project" / VERSION
    inputs = work_dir / "inputs"
    inputs.mkdir(parents=True, exist_ok=True)
    default_archive = archive is None
    if default_archive:
        archive = vendor / f"{name}.tar.xz"
        if not archive.is_file():
            archive = inputs / archive.name
        if not archive.is_file():
            url = (
                "https://github.com/llvm/llvm-project/releases/download/"
                f"llvmorg-{VERSION}/{archive.name}"
            )
            print(f"Downloading {url}", flush=True)
            staged = archive.with_suffix(".xz.new")
            try:
                with urllib.request.urlopen(url, timeout=60) as response, staged.open("wb") as out:
                    shutil.copyfileobj(response, out)
                staged.replace(archive)
            finally:
                staged.unlink(missing_ok=True)
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
