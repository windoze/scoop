"""Keep native sample counts, symbols, offsets and unknown frames intact."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

BENCHMARKS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCHMARKS))

import symbolize  # noqa: E402


class SymbolizeTests(unittest.TestCase):
    first = "scoop$1$cb$" + "a" * 64
    second = "scoop$1$cb$" + "b" * 64

    def test_perf_and_sample_keep_weights_offsets_and_distinct_instances(self):
        text = f"  42.00% {self.first}+0x14\n  + 37 _{self.second} (in program) + 48\n"
        result = symbolize.annotate(text, {self.first: "Box.get", self.second: "Box.get"})
        self.assertEqual(
            result,
            f"  42.00% Box.get {{{self.first}}}+0x14\n"
            f"  + 37 Box.get {{_{self.second}}} (in program) + 48\n",
        )

    def test_unknown_symbols_clones_and_partial_tokens_remain_unchanged(self):
        text = "\n".join(
            [
                self.second,
                self.first + ".llvm.123",
                self.first + ".cold",
                self.first + "a",
                "prefix_" + self.first,
                "__" + self.first,
                "scoop_gc_poll+0x10",
                "0x123456",
            ]
        )
        self.assertEqual(symbolize.annotate(text, {self.first: "answer"}), text)

    def test_multiple_maps_accept_shared_bodies_and_reject_conflicting_labels(self):
        with self.maps() as directory:
            root = Path(directory)
            first = self.write_map(root / "one.json", self.first, "get")
            second = self.write_map(root / "two.json", self.first, "get")
            self.assertEqual(symbolize.labels([first, second]), {self.first: "get"})
            self.write_map(second, self.first, "other")
            with self.assertRaisesRegex(ValueError, "conflicting labels"):
                symbolize.labels([first, second])

    def test_generated_roles_and_missing_declarations(self):
        with self.maps() as directory:
            path = Path(directory) / "symbols.json"
            self.write_map(path, self.first, None, "boxing-adjust")
            self.assertEqual(symbolize.labels([path]), {self.first: "<boxing-adjust>"})
            self.write_map(path, self.first, "Base.value", "dispatch-adjust")
            self.assertEqual(symbolize.labels([path]), {self.first: "Base.value [dispatch-adjust]"})

    def test_cli_preserves_raw_input_and_reports_bad_maps(self):
        with self.maps() as directory:
            root = Path(directory)
            mapping = self.write_map(root / "symbols.json", self.first, "answer")
            source, output = root / "perf.txt", root / "annotated.txt"
            original = f"99.0% {self.first}+0x4\n"
            source.write_text(original)
            command = [
                sys.executable,
                str(BENCHMARKS / "symbolize.py"),
                "--symbols",
                str(mapping),
                "--input",
                str(source),
                "--output",
                str(output),
            ]
            completed = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(source.read_text(), original)
            self.assertIn(f"answer {{{self.first}}}+0x4", output.read_text())
            command[-1] = str(source)
            completed = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(completed.returncode, 0)
            self.assertEqual(source.read_text(), original)
            command[-1] = str(output)
            mapping.write_text('{"schema": 2}')
            completed = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("expected symbols schema 1", completed.stderr)

    def maps(self):
        root = BENCHMARKS.parents[1] / "tmp"
        root.mkdir(exist_ok=True)
        return tempfile.TemporaryDirectory(prefix="symbols-test-", dir=root)

    def write_map(self, path, symbol, name, role="callable"):
        path.write_text(
            json.dumps(
                {
                    "schema": 1,
                    "symbols": [{"symbol": symbol, "name": name, "role": role}],
                }
            )
        )
        return path


if __name__ == "__main__":
    unittest.main()
