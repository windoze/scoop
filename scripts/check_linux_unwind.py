#!/usr/bin/env python3
"""Run the actual Scoop personality with LLVM libunwind for all Linux modes."""

import argparse
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sysroot", type=Path, default=ROOT / "sysroot")
    parser.add_argument("--llvm-prefix", type=Path, default=os.environ.get("LLVM_SYS_221_PREFIX"))
    parser.add_argument("--gnu-cc", default="gcc")
    parser.add_argument("--musl-cc", default="musl-gcc")
    args = parser.parse_args()
    if args.llvm_prefix is None:
        parser.error("set LLVM_SYS_221_PREFIX or --llvm-prefix to LLVM 22.1")
    llc = args.llvm_prefix.resolve() / "bin/llc"
    version = subprocess.check_output([llc, "--version"], text=True)
    if "LLVM version 22.1." not in version:
        parser.error(f"{llc} must be LLVM 22.1")
    modes = [
        ("gnu", "dynamic", args.gnu_cc, ["-fPIE", "-pie"]),
        ("musl", "static", args.musl_cc, ["-static", "-fno-pie", "-no-pie"]),
        ("musl", "dynamic", args.musl_cc, ["-fPIE", "-pie"]),
    ]
    for libc, mode, cc, flags in modes:
        triple = f"x86_64-unknown-linux-{libc}"
        prefix = args.sysroot.resolve() / "native" / triple / "unwind"
        build = ROOT / "target/unwind-tests" / triple / mode
        build.mkdir(parents=True, exist_ok=True)
        environment = dict(os.environ, TMPDIR=str(build))
        obj, binary, link_map = build / "frames.o", build / "unwind-test", build / "link.map"
        subprocess.run(
            [
                llc,
                f"-mtriple={triple}",
                "-O=2",
                "-relocation-model=pic",
                "-filetype=obj",
                str(ROOT / "runtime/tests/eh_unwind_test.ll"),
                "-o",
                str(obj),
            ],
            check=True,
            env=environment,
        )
        subprocess.run(
            [
                cc,
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O2",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
                "-funwind-tables",
                "-I",
                str(prefix / "include"),
                "-I",
                str(ROOT / "runtime/src"),
                str(ROOT / "runtime/tests/eh_unwind_test.c"),
                str(ROOT / "runtime/src/eh_personality.c"),
                str(ROOT / "runtime/src/eh/lsda.c"),
                str(obj),
                str(prefix / "lib/libunwind.a"),
                "-pthread",
                "-nodefaultlibs",
                "-Wl,--eh-frame-hdr",
                f"-Wl,-Map,{link_map}",
                *flags,
                "-lc",
                "-lgcc",
                "-o",
                str(binary),
            ],
            check=True,
            env=environment,
        )
        result = subprocess.check_output([binary], text=True, env=environment)
        if result != "cleanup, catch and delete passed\n":
            raise RuntimeError(f"unexpected EH result for {triple}/{mode}: {result!r}")
        inputs = link_map.read_text()
        if "libgcc_eh.a" in inputs or "libgcc_s.so" in inputs:
            raise RuntimeError(f"a second unwinder was linked: {link_map}")
        elf = subprocess.check_output(["readelf", "-l", "-d", binary], text=True)
        if mode == "static":
            if "INTERP" in elf or "NEEDED" in elf:
                raise RuntimeError(f"static output has dynamic dependencies: {binary}")
        elif "INTERP" not in elf or ("ld-musl-" in elf) != (libc == "musl"):
            raise RuntimeError(f"incorrect libc interpreter: {binary}")
        print(f"{triple}/{mode}: {result.strip()}")


if __name__ == "__main__":
    main()
