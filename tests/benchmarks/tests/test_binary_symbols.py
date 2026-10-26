"""Resolve actual text aliases without choosing between coalesced bodies."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import binary_symbols  # noqa: E402
import symbolize  # noqa: E402


class BinarySymbolsTests(unittest.TestCase):
    def test_elf_and_macho_start_aliases_keep_original_symbol_and_offset(self):
        body = "scoop$1$cb$" + "a" * 64
        boundary = "scoop$1$bs$" + "b" * 64
        for prefix in ("", "_"):
            with self.subTest(prefix=prefix):
                text = f"{prefix}{body} W 1230 40\n{prefix}{boundary} t 1230 0\n"
                names = binary_symbols.aliases(text, {body: "sumNodes"})
                raw = f"99.0% {prefix}{boundary}+0xffffabcd\n"
                self.assertEqual(
                    symbolize.annotate(raw, names),
                    f"99.0% sumNodes [body={body}] {{{prefix}{boundary}}}+0xffffabcd\n",
                )

    def test_coalesced_bodies_data_and_unknown_addresses_are_not_guessed(self):
        first, second = ("scoop$1$cb$" + digit * 64 for digit in ("a", "b"))
        boundary = "scoop$1$bs$" + "c" * 64
        names = {first: "first", second: "second"}
        for records in (
            f"{first} T 1230 20\n{second} T 1230 20\n{boundary} t 1230 0\n",
            f"{first} T 1200 20\n{boundary} t 1230 0\n",
            f"{first} T 1230 20\n{boundary} D 1230 0\n",
        ):
            self.assertEqual(binary_symbols.aliases(records, names), names)
        partial = {first: "first"}
        for other in (second, first + ".llvm.123"):
            records = f"{first} T 1230 20\n{other} T 1230 20\n{boundary} t 1230 0\n"
            self.assertEqual(binary_symbols.aliases(records, partial), partial)


if __name__ == "__main__":
    unittest.main()
