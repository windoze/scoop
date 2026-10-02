"""Source encoding errors belong to the compiler, not fixture discovery."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.discovery import discover
from fixture_runner.model import ConfigurationError

CRITERIA = """schema = 1
name = "source-encoding"
inputs = ["main.scoop"]
[[steps]]
name = "compile"
argv = ["${scoop}", "build", "${fixture}/main.scoop"]
exit = 1
stdout = ""
stderr = ""
"""


class DiscoveryTests(unittest.TestCase):
    def test_invalid_source_bytes_are_preserved_with_both_carriers(self):
        body = b"fun main() {\xff}\n"
        for inline in (False, True):
            with self.subTest(inline=inline), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                source = base / "main.scoop"
                if inline:
                    header = "// fixture:begin\n"
                    header += "".join("// " + line + "\n" for line in CRITERIA.splitlines())
                    original = (header + "// fixture:end\n").encode() + body
                else:
                    (base / "main.fixture.toml").write_text(CRITERIA)
                    original = body
                source.write_bytes(original)
                fixtures = discover(base)
                self.assertEqual(len(fixtures), 1)
                self.assertEqual(fixtures[0].inputs, {source.resolve()})
                self.assertEqual(source.read_bytes(), original)

    def test_invalid_criteria_encoding_is_still_a_configuration_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "main.scoop"
            source.write_bytes(b"// fixture:begin\n// name = '\xff'\n// fixture:end\n")
            with self.assertRaisesRegex(ConfigurationError, "inline criteria must be UTF-8"):
                discover(source.parent)
