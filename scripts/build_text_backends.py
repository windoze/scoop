#!/usr/bin/env python3
"""Prepare the fixed Rust text backend for one Scoop target."""

import argparse
import os
import platform
from pathlib import Path

import build_rust_text
from build_support import ROOT
from target_catalog import NATIVE_TARGETS as TARGETS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host, _ = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    build_rust_text.prepare(args)


if __name__ == "__main__":
    main()
