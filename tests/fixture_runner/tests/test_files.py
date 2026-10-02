"""Corruption operations change only their declared file and byte range."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.files import prepare
from fixture_runner.model import ConfigurationError


class FileTests(unittest.TestCase):
    def test_copy_patch_and_truncate_preserve_the_original(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "original").write_bytes(b"abcdefgh")
            prepare(
                [
                    {"copy": {"from": "original", "to": "damaged"}},
                    {"patch": {"path": "damaged", "offset": 2, "hex": "0001"}},
                    {"truncate": {"path": "damaged", "size": 6}},
                ],
                {},
                base,
            )
            self.assertEqual((base / "damaged").read_bytes(), b"ab\0\x01ef")
            self.assertEqual((base / "original").read_bytes(), b"abcdefgh")

    def test_out_of_bounds_truncation_does_not_change_existing_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            target = base / "artifact"
            target.write_bytes(b"original")
            for size in (-1, 9):
                with self.subTest(size=size), self.assertRaises(ConfigurationError):
                    prepare([{"truncate": {"path": "artifact", "size": size}}], {}, base)
                self.assertEqual(target.read_bytes(), b"original")
            prepare([{"truncate": {"path": "artifact", "size": 0}}], {}, base)
            self.assertEqual(target.read_bytes(), b"")
