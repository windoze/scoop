"""Select the real target-specific built-in Cone graph for builds and documentation."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_documentation
from prepare_sysroot import cone_order, has_target_sources

DARWIN = "aarch64-apple-darwin"
LINUX = "x86_64-unknown-linux-gnu"


class SysrootSelectionTests(unittest.TestCase):
    def write_cone(self, root, name, extra=""):
        path = root / "src" / name / "Cone.toml"
        path.parent.mkdir(parents=True)
        path.write_text(f'[cone]\ngroup = "scoop"\nname = "{name}"\n' + extra)

    def test_target_graph_reuses_and_updates_selection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_cone(root, "scoop.core")
            self.write_cone(root, "scoop.std")
            self.write_cone(
                root,
                "scoop.window",
                '[[sources]]\npath = "src"\nwhen = { os = "darwin" }\n'
                '[dependencies]\n"scoop:scoop.std" = "0.1.0"\n',
            )
            expected = ["scoop.core", "scoop.std", "scoop.window"]
            self.assertEqual(cone_order(root, root, DARWIN), expected)
            with patch("prepare_sysroot.tomllib.load", side_effect=AssertionError("cache miss")):
                self.assertEqual(cone_order(root, root, DARWIN), expected)
            self.assertEqual(cone_order(root, root, LINUX), expected[:2])
            self.write_cone(
                root, "scoop.consumer", '[dependencies]\n"scoop:scoop.window" = "0.1.0"\n'
            )
            with self.assertRaisesRegex(ValueError, "scoop:scoop.window has no source Cone"):
                cone_order(root, root, LINUX)

    def test_conditions_and_empty_selection_keep_compiler_responsibility(self):
        self.assertTrue(has_target_sources({}, LINUX))
        self.assertTrue(has_target_sources({"sources": []}, LINUX))
        sources = {"sources": [{"path": "src", "when": {"os": ["darwin", "linux"], "env": "none"}}]}
        self.assertTrue(has_target_sources(sources, DARWIN))
        self.assertFalse(has_target_sources(sources, LINUX))
        with self.assertRaisesRegex(ValueError, "unknown target predicate"):
            has_target_sources({"sources": [{"path": "src", "when": {"os": "dawr in"}}]}, DARWIN)

    def test_documentation_uses_the_same_target_graph(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_cone(root, "scoop.core")
            self.write_cone(
                root, "scoop.window", '[[sources]]\npath = "src"\nwhen = {os = "darwin"}\n'
            )
            with patch("package_documentation.subprocess.run") as run:
                package_documentation.generate(root, [DARWIN, LINUX], ["scoop", "scoopc"], root, {})
            first, second = [call.args[0] for call in run.call_args_list]
            window = str(root / "src/scoop.window/Cone.toml")
            self.assertIn(window, first)
            self.assertNotIn(window, second)
