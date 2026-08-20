"""Corruption operations change only their declared file and byte range."""

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.files import prepare
from fixture_runner.model import ConfigurationError


class FileTests(unittest.TestCase):
    def test_concat_preserves_bytes_and_explicit_separators(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            original = "源码\r\n".encode()
            (base / "first").write_bytes(original)
            (base / "second").write_bytes(b"\x80\x00\xff")
            prepare(
                [
                    {
                        "concat": {
                            "path": "out/combined",
                            "parts": [{"file": "first"}, "\n", {"file": "second"}, {"hex": "00ff"}],
                        }
                    }
                ],
                {},
                base,
            )
            self.assertEqual(
                (base / "out/combined").read_bytes(), original + b"\n\x80\x00\xff\x00\xff"
            )
            self.assertEqual((base / "first").read_bytes(), original)
            self.assertEqual((base / "second").read_bytes(), b"\x80\x00\xff")

    def test_concat_reads_inputs_before_replacement_and_composes_with_patch(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            target = base / "output"
            target.write_bytes(b"original")
            with self.assertRaises(FileNotFoundError):
                prepare(
                    [
                        {
                            "concat": {
                                "path": "output",
                                "parts": [{"file": "output"}, {"file": "missing"}],
                            }
                        }
                    ],
                    {},
                    base,
                )
            self.assertEqual(target.read_bytes(), b"original")
            prepare(
                [
                    {"concat": {"path": "output", "parts": [{"file": "output"}, "${suffix}"]}},
                    {"patch": {"path": "output", "offset": 8, "hex": "ff"}},
                ],
                {"suffix": b"\x00\x01"},
                base,
            )
            self.assertEqual(target.read_bytes(), b"original\xff\x01")

    def test_remove_unlinks_fifo_and_symlink_without_reading_the_target(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            os.mkfifo(base / "index.fifo")
            (base / "original").write_bytes(b"preserved")
            (base / "alias").symlink_to(base / "original")
            prepare(
                [{"remove": {"path": "index.fifo"}}, {"remove": {"path": "alias"}}],
                {},
                base,
            )
            self.assertFalse((base / "index.fifo").exists())
            self.assertFalse((base / "alias").is_symlink())
            self.assertEqual((base / "original").read_bytes(), b"preserved")

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
