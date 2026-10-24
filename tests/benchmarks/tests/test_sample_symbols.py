"""Keep collapsed sample frames intact while resolving only explicit main-image PCs."""

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sample_symbols  # noqa: E402


class SampleSymbolsTests(unittest.TestCase):
    def test_aslr_and_collapsed_addresses_do_not_redistribute_counts(self):
        text = """Load Address: 0x100600000
Call graph:
  10 <deduplicated_symbol> (in program) + 4,8,... [0x100600104,0x100600208,...]
    3 native (in library) + 4 [0x200000004]
Sort by top of stack, same collapsed:
  <deduplicated_symbol> 10
Binary Images:
  0x100600000 - 0x10060ffff +program (0)
  0x200000000 - 0x20000ffff library (0)
"""
        first, second = "scoop$1$cb$" + "a" * 64, "scoop$1$cb$" + "b" * 64
        names = {first: "exercise", second: "sumNodes"}
        results = [
            {
                "Address": "0x100600104",
                "Symbol": [{"FunctionName": first, "StartAddress": "0x100000100"}],
            },
            {
                "Address": "0x100600208",
                "Symbol": [{"FunctionName": second, "StartAddress": "0x100000200"}],
            },
        ]
        output = "\n".join(json.dumps(result) for result in results)
        frames = sample_symbols.resolved_frames(output, names, 0x600000)
        start, end = sample_symbols.image_range(text)
        annotated = sample_symbols.append_frames(text, frames, start, end)
        self.assertIn("10 <deduplicated_symbol>", annotated)
        self.assertIn("+ 4,8,... [0x100600104,0x100600208,...]", annotated)
        self.assertIn(f"0x100600104 = exercise {{{first}}}+0x4", annotated)
        self.assertIn(f"0x100600208 = sumNodes {{{second}}}+0x8", annotated)
        self.assertIn("3 native (in library) + 4 [0x200000004]\n", annotated)
        self.assertIn("<deduplicated_symbol> 10\n", annotated)

    def test_unknown_symbols_and_missing_image_information(self):
        result = json.dumps([{"Address": "0x100000100", "Symbol": [{"FunctionName": "unknown"}]}])
        self.assertEqual(sample_symbols.resolved_frames(result, {}, 0), {})
        with self.assertRaisesRegex(ValueError, "Load Address"):
            sample_symbols.image_range("no header")
        with self.assertRaisesRegex(ValueError, "Binary Images"):
            sample_symbols.image_range("Load Address: 0x100000000\n")
        with self.assertRaisesRegex(ValueError, "__TEXT"):
            sample_symbols.text_base("Segment { Name: __PAGEZERO\nvmaddr: 0x0\n}")
        self.assertEqual(
            sample_symbols.text_base("Segment {\nName: __TEXT\nvmaddr: 0x100000000\n}"),
            0x100000000,
        )


if __name__ == "__main__":
    unittest.main()
