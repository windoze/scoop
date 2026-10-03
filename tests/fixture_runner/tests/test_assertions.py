"""Membership checks retain explicit negative symbol and link-plan assertions."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.assertions import check_all
from fixture_runner.model import AssertionFailure


class AssertionTests(unittest.TestCase):
    def test_absent_symbols_and_rpaths_are_checked_in_text_and_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "symbols").write_bytes(b"entry\nhelper\n")
            checks = [
                {"file": "symbols", "not_contains": "unused"},
                {"actual": "${link.plan}", "contains": "dynamic binding"},
                {"actual": "${link.plan}", "not_contains": "\nrpath "},
            ]
            context = {"link": {"plan": "dynamic binding entry -> /absolute/library\n"}}
            check_all(checks, context, base, False)
            context["link"]["plan"] += 'rpath "/unexpected"\n'
            with self.assertRaises(AssertionFailure):
                check_all(checks, context, base, False)
            (base / "symbols").write_bytes(b"entry\nunused\n")
            with self.assertRaises(AssertionFailure):
                check_all(checks, {}, base, False)
