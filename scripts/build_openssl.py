#!/usr/bin/env python3
"""Prepare Scoop's vendored OpenSSL headers and static libraries for one target."""

import argparse
import os
import platform
import shlex
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, build_lock, metadata, reusable, save_record
from target_catalog import NATIVE_TARGETS, native_configuration

VERSION = "3.5.9"
# Configure names are OpenSSL recipe choices, keyed by actual machine/environment.
CONFIGURATIONS = {
    ("Darwin", "aarch64"): "darwin64-arm64-cc",
    ("LinuxGnu", "x86_64"): "linux-x86_64",
    ("LinuxMusl", "x86_64"): "linux-x86_64",
    ("MicrosoftUcrt", "x86_64"): "VC-WIN64A",
    ("LlvmMingwMsvcrt", "x86_64"): "mingw64",
    ("LlvmMingwUcrt", "x86_64"): "mingw64",
}
TARGETS = {
    name: (CONFIGURATIONS[(target["native_environment"], target["arch"])], cc, host)
    for name, (host, cc) in NATIVE_TARGETS.items()
    for target in [native_configuration(name)]
}

OPTIONS = [
    "no-shared",
    "no-module",
    "no-dso",
    "no-engine",
    "no-autoload-config",
    "no-tests",
    "no-apps",
    "no-docs",
]


def install_msvc(source, objects, prefix):
    headers = prefix / "include/openssl"
    files = [
        (header, headers / header.name)
        for directory in (source, objects)
        for header in sorted((directory / "include/openssl").glob("*.h"))
        if not header.name.startswith("__DECC_")
    ]
    files.extend(
        (objects / f"lib{name}.lib", prefix / "lib" / f"lib{name}.a") for name in ("ssl", "crypto")
    )
    pdb = objects / "ossl_static.pdb"
    if pdb.exists():
        files.append((pdb, prefix / "lib" / pdb.name))
    # clang-cl embeds CodeView in the objects and does not emit the compiler PDB
    # that upstream's NMake install_dev unconditionally tries to copy.
    for source_file, destination in files:
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_suffix(destination.suffix + ".new")
        shutil.copy2(source_file, temporary)
        temporary.replace(destination)


def prepare(args):
    config, default_cc, host = TARGETS[args.target]
    msvc = config == "VC-WIN64A"
    mingw = config == "mingw64"
    archive = ROOT / f"sysroot/vendor/openssl/openssl-{VERSION}.tar.gz"
    build = args.build_dir
    prefix = args.prefix
    inputs = {
        "archive": metadata(archive),
        "scripts": [
            metadata(path) for path in (__file__, RELEASE_CONFIG, ROOT / "scripts/build_support.py")
        ],
        "target": args.target,
        "target_configuration": native_configuration(args.target),
        "config": config,
        "cc": args.cc or default_cc,
        "perl": args.perl,
        "make": args.make or ("nmake" if msvc else "make"),
        "shell": args.shell or ("sh" if mingw else None),
        "prefix": str(prefix),
        "options": OPTIONS,
        "environment": {
            name: os.environ.get(name)
            for name in (
                "PATH",
                "CFLAGS",
                "CPPFLAGS",
                "LDFLAGS",
                "AR",
                "RANLIB",
                "PERL5LIB",
                "INCLUDE",
                "LIB",
                "LIBPATH",
                "CL",
                "_CL_",
                "MACOSX_DEPLOYMENT_TARGET",
                "MSYS2_ENV_CONV_EXCL",
            )
        },
    }
    record = build / "inputs.json"
    outputs = [
        prefix / name for name in ("include/openssl/ssl.h", "lib/libssl.a", "lib/libcrypto.a")
    ]
    if not args.rebuild and reusable(record, inputs, outputs):
        print(f"Reusing {args.target} OpenSSL {VERSION}: {prefix}", flush=True)
        return

    tools = {}
    for name in ("cc", "perl", "make", "shell"):
        if inputs[name] is not None:
            path = shutil.which(inputs[name])
            if path is None:
                raise ValueError(f"OpenSSL preparation requires {name}: {inputs[name]}")
            tools[name] = Path(path).absolute().as_posix()
    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    flags = [] if host == "Windows" else ["-fPIC"]
    if host == "Darwin":
        baseline = release["minimum_os"]
        flags.append("-mmacosx-version-min=" + os.environ.get("MACOSX_DEPLOYMENT_TARGET", baseline))
    elif msvc:
        flags.extend(("/MD", "/D_WIN32_WINNT=0x0a00"))
    else:
        flags.append("-march=" + release["cpu"])
        if mingw:
            flags.append("-D_WIN32_WINNT=0x0a00")
    scratch = build / "scratch"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(
        os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch), CC=tools["cc"]
    )
    make = [tools["make"]]
    configure_prefix = prefix.as_posix()
    if host == "Windows":
        environment["PATH"] = str(Path(tools["cc"]).parent) + os.pathsep + environment["PATH"]
    if msvc:
        environment["VSLANG"] = "1033"
        make.append("/NOLOGO")
        for name in ("nasm", "lib", "link"):
            path = shutil.which(name, path=environment["PATH"])
            if path is None:
                raise ValueError(f"Windows OpenSSL preparation requires {name}")
            tools[name] = path
    if mingw:
        environment["CC"] = shlex.quote(tools["cc"])
        for name in ("ar", "ranlib"):
            path = Path(tools["cc"]).parent / ("llvm-" + name + ".exe")
            if not path.is_file():
                raise ValueError(f"the selected LLVM-MinGW prefix is missing {path.name}")
            tools[name] = path.as_posix()
            environment[name.upper()] = shlex.quote(tools[name])
        environment["SHELL"] = tools["shell"]
        # A native GNU Make must also invoke the selected MSYS Perl and shell.
        make.extend((f"SHELL={tools['shell']}", f"PERL={shlex.quote(tools['perl'])}"))
        prefix.mkdir(parents=True, exist_ok=True)
        configure_prefix = subprocess.check_output(
            [tools["perl"], "-MCwd=abs_path", "-e", "print abs_path($ARGV[0])", str(prefix)],
            env=environment,
            text=True,
            encoding="utf-8",
        )
    sources = build / "source"
    source = sources / f"openssl-{VERSION}"
    # The complete upstream archive remains the source input for every target.
    with tarfile.open(archive) as upstream:
        upstream.extractall(sources, filter="data")
    objects = build / "objects"
    objects.mkdir(exist_ok=True)
    if (objects / "Makefile").exists():
        subprocess.run([*make, "clean"], cwd=objects, env=environment, check=True)
    command = [
        tools["perl"],
        (source / "Configure").as_posix(),
        config,
        f"--prefix={configure_prefix}",
        "--libdir=lib",
        *OPTIONS,
        *flags,
    ]
    subprocess.run(command, cwd=objects, env=environment, check=True)
    subprocess.run(
        [*make, *([] if msvc else [f"-j{args.jobs}"]), "build_libs"],
        cwd=objects,
        env=environment,
        check=True,
    )
    if msvc:
        install_msvc(source, objects, prefix)
    else:
        subprocess.run([*make, "install_dev"], cwd=objects, env=environment, check=True)
    save_record(record, inputs, objects, tools=tools.values())
    print(f"Installed {args.target} OpenSSL {VERSION}: {prefix}", flush=True)
    print(f"Default Scoop library directory: {prefix / 'lib'}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc", help="Compiler for the selected target libc")
    parser.add_argument("--perl", default="perl")
    parser.add_argument("--make")
    parser.add_argument("--shell", help="Unix shell for the LLVM-MinGW build")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 8))
    parser.add_argument("--rebuild", action="store_true")
    args = parser.parse_args()
    host = TARGETS[args.target][2]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if "llvm-mingw" in args.target and not args.cc:
        parser.error("--cc must select clang.exe from the LLVM-MinGW prefix for this CRT")
    args.build_dir = (args.build_dir or ROOT / "tmp/openssl" / args.target).resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target / "openssl").resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with build_lock(args.build_dir / "build.lock"):
        prepare(args)


if __name__ == "__main__":
    main()
