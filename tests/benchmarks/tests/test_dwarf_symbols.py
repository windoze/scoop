"""Preserve exact identities, source locations and inline chains from LLVM JSON."""

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import dwarf_symbols  # noqa: E402


class DwarfSymbolsTests(unittest.TestCase):
    def test_inline_frames_have_locations_without_name_based_identity_merging(self):
        first, second = "scoop$1$cb$" + "a" * 64, "scoop$1$cb$" + "b" * 64
        records = [
            {
                "Address": "0x6020",
                "Symbol": [
                    {"FunctionName": first, "FileName": "/source/inline.scoop", "Line": 3},
                    {
                        "FunctionName": second,
                        "StartAddress": "0x1000",
                        "FileName": "/source/main.scoop",
                        "Line": 8,
                    },
                ],
            }
        ]
        frames = dwarf_symbols.resolved_frames(
            json.dumps(records), {first: "overload", second: "overload"}, 0x5000
        )
        self.assertEqual(
            frames[0x6020],
            f"overload {{{first}}} (/source/inline.scoop:3) [inline] <- overload {{{second}}}+0x20 (/source/main.scoop:8)",
        )

    def test_inline_display_name_is_not_used_to_look_up_entity_identity(self):
        body = "scoop$1$cb$" + "c" * 64
        records = {
            "Address": "0x10",
            "Symbol": [
                {"FunctionName": "sameName", "FileName": "inline.scoop", "Line": 4},
                {"FunctionName": body, "FileName": "main.scoop", "Line": 10},
            ],
        }
        frames = dwarf_symbols.resolved_frames(json.dumps(records), {body: "sameName"}, 0)
        self.assertTrue(frames[0x10].startswith("sameName (inline.scoop:4) [inline] <-"))
        self.assertEqual(frames[0x10].count("{"), 1)
        records["Symbol"] = [{"FunctionName": "??"}]
        self.assertEqual(dwarf_symbols.resolved_frames(json.dumps(records), {}, 0), {})


if __name__ == "__main__":
    unittest.main()
