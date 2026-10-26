"""Resolve real linker ranges without assigning gaps or ambiguous aliases."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import link_map  # noqa: E402


class LinkMapTests(unittest.TestCase):
    first = "scoop$1$cb$" + "a" * 64
    second = "scoop$1$cb$" + "b" * 64

    def test_darwin_sized_atoms_aliases_and_dead_symbols(self):
        text = f"""# Sections:
# Address Size Segment Section
0x100001000 0x00001000 __TEXT __text
# Symbols:
# Address Size File Name
0x100001000 0x00000000 [ 1] _{self.first}
0x100001000 0x00000020 [ 1] _scoop$1$bs${"c" * 64}
0x100001080 0x00000000 [ 1] _{self.second}
# Dead Stripped Symbols:
0x100001040 0x00000020 [ 2] _{self.second}
"""
        addresses = link_map.AddressMap(link_map.darwin_ranges(text))
        self.assertEqual(addresses.resolve(0x100001004), (self.first, 4))
        self.assertIsNone(addresses.resolve(0x100001020))
        self.assertIsNone(addresses.resolve(0x100001044))
        self.assertIsNone(addresses.resolve(0x100001080))

    def test_elf_multiline_input_sections_end_markers_and_gaps(self):
        text = f"""Discarded input sections
 .text 0x0 0x100 dead.o
                0x0 {self.second}
Linker script and memory map
.text 0x1000 0x400
 .text 0x1000 0x100 first.o
                0x1000 {self.first}
                0x1020 scoop$1$be${"c" * 64}
 .text.long_input_section_name
                0x1100 0x20 second.o
                0x1100 {self.second}
 .rodata 0x2000 0x20 data.o
                0x2000 {self.first}
"""
        addresses = link_map.AddressMap(link_map.elf_ranges(text))
        self.assertEqual(addresses.resolve(0x1004), (self.first, 4))
        self.assertEqual(addresses.resolve(0x111F), (self.second, 31))
        for address in [0, 0x1020, 0x1080, 0x1120, 0x2000]:
            self.assertIsNone(addresses.resolve(address))

    def test_ambiguous_overlaps_are_not_assigned_and_aslr_is_explicit(self):
        addresses = link_map.AddressMap(
            [
                (0x1000, 0x1040, self.first),
                (0x1010, 0x1020, self.second),
                (0x1080, 0x1090, self.second),
            ]
        )
        self.assertIsNone(addresses.resolve(0x1014))
        self.assertEqual(addresses.resolve(0x1024), (self.first, 36))
        self.assertEqual(
            addresses.frames([0x701004, 0x701014, 0x701084], {self.first: "run"}, 0x700000),
            {0x701004: f"run {{{self.first}}}+0x4"},
        )


if __name__ == "__main__":
    unittest.main()
