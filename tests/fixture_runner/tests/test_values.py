"""Raw arguments and JSON path carriers use the same declared byte references."""

import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.values import argv_values, expand, path_bytes


class ValueTests(unittest.TestCase):
    def test_text_and_bytes_hex_compose_raw_arguments_and_json_paths(self):
        context = {"work": "/tmp/中文 path", "suffix": b"/source-\xff.scoop"}
        encoded = "${work.hex}${suffix.hex}"
        expected = os.fsencode(context["work"]) + context["suffix"]
        self.assertEqual(argv_values([{"hex": encoded}], context), [expected])
        carrier = expand({"encoding": "unix-bytes", "hex": encoded}, context)
        self.assertEqual(path_bytes(carrier), expected)

    def test_hex_fields_in_json_objects_retain_their_ordinary_meaning(self):
        context = {"value": {"hex": "ff"}, "rows": [b"\x00\xfe"]}
        self.assertEqual(expand("${value.hex}", context), "ff")
        self.assertEqual(expand("${rows.0.hex}", context), "00fe")
