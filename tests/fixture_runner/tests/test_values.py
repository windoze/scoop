"""Raw arguments and JSON path carriers use the same declared byte references."""

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.values import argv_values, expand, path_bytes, references


class ValueTests(unittest.TestCase):
    def test_canonical_reference_names_the_actual_host_input(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "nested").mkdir()
            source = base / "值.txt"
            source.write_bytes(b"source")
            value = expand("${source.canonical}", {"source": base / "nested/../值.txt"})
            self.assertEqual(Path(value).read_bytes(), b"source")
            self.assertNotIn("..", Path(value).parts)
            if os.name == "nt":
                self.assertTrue(value.startswith("\\\\?\\"))
            self.assertEqual(expand("${record.canonical}", {"record": {"canonical": 7}}), 7)

    def test_host_paths_cover_missing_inputs_and_cli_executable_names(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            value = expand("${source.canonical}", {"source": base / "missing.scoop"})
            self.assertFalse(Path(value).exists())
            self.assertTrue(Path(value).parent.samefile(base))
            self.assertEqual(expand("${tool.filename}", {"tool": base / "sco.exe"}), "sco.exe")
            self.assertEqual(expand("${tool.filename}", {"tool": base / "scoop"}), "scoop")
            self.assertEqual(expand("${record.filename}", {"record": {"filename": 7}}), 7)

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

    def test_escaped_diagnostics_do_not_require_a_reference_binding(self):
        expected = [{"message": "use `$${name}`; $$$$ keeps two dollars"}]
        self.assertEqual(list(references(expected)), [])
        self.assertEqual(
            expand(expected, {}),
            [{"message": "use `${name}`; $$ keeps two dollars"}],
        )

    def test_literal_dollar_and_reference_compose_native_symbol_names(self):
        value = "_scoop$1$im$$${provider.identity}"
        self.assertEqual(list(references(value)), ["provider.identity"])
        self.assertEqual(expand(value, {"provider": {"identity": "abc"}}), "_scoop$1$im$abc")

    def test_expansion_preserves_typed_values_and_does_not_rescan_replacements(self):
        context = {"diagnostic": "${unbound}", "rows": [1, 2]}
        self.assertEqual(expand("${rows}", context), [1, 2])
        self.assertEqual(expand("error: ${diagnostic}", context), "error: ${unbound}")
        self.assertEqual(expand("${diagnostic}", context), "${unbound}")
