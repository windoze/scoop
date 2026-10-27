#!/usr/bin/env python3
"""Run the repository's declarative fixture suite (Python 3.11+)."""

import os
import sys

from fixture_runner.cli import main

if __name__ == "__main__":
    if os.name == "nt":
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    raise SystemExit(main())
