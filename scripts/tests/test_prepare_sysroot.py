"""Select the real target-specific built-in Cone graph for builds and documentation."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import prepare_sysroot
from prepare_sysroot import cone_order, has_target_sources
from target_catalog import TARGET_FACTS

DARWIN = "aarch64-apple-darwin"
LINUX = "x86_64-unknown-linux-gnu"
WINDOWS = "x86_64-pc-windows-msvc-ucrt"


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
            self.assertEqual(cone_order(root, root, WINDOWS), expected[:2])
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
        self.assertFalse(has_target_sources(sources, WINDOWS))
        windows = {"sources": [{"path": "src", "when": {"os": "windows", "env": "msvc-ucrt"}}]}
        self.assertTrue(has_target_sources(windows, WINDOWS))
        self.assertFalse(has_target_sources(windows, DARWIN))
        with self.assertRaisesRegex(ValueError, "unknown target predicate"):
            has_target_sources({"sources": [{"path": "src", "when": {"os": "dawr in"}}]}, DARWIN)

    def test_catalog_update_recomputes_selected_cones(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_cone(root, "scoop.core")
            self.write_cone(
                root, "scoop.machine", '[[sources]]\npath = "src"\nwhen = {arch = "x86_64"}\n'
            )
            self.assertEqual(cone_order(root, root, DARWIN), ["scoop.core"])
            changed = {**TARGET_FACTS, DARWIN: {**TARGET_FACTS[DARWIN], "arch": "x86_64"}}
            with patch("prepare_sysroot.TARGET_FACTS", changed):
                self.assertEqual(cone_order(root, root, DARWIN), ["scoop.core", "scoop.machine"])

    def test_runtime_only_does_not_select_or_build_system_cones(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            include = root / "runtime/include"
            include.mkdir(parents=True)
            for name in ("scoop_rt.h", "scoop_runtime_metadata_v1.h"):
                (include / name).write_text("/* Runtime header. */\n")
            arguments = [
                "prepare_sysroot.py",
                "--runtime-only",
                "--target",
                DARWIN,
                "--sysroot",
                str(root / "sysroot"),
                "--runtime-root",
                str(root / "runtime"),
                "--work-dir",
                str(root / "work"),
            ]
            with (
                patch.object(sys, "argv", arguments),
                patch.object(prepare_sysroot, "runtime") as runtime,
                patch.object(
                    prepare_sysroot, "cone_order", side_effect=AssertionError("selected Cones")
                ),
                patch.object(
                    prepare_sysroot, "build_cones", side_effect=AssertionError("built Cones")
                ),
            ):
                prepare_sysroot.main()
            self.assertEqual(runtime.call_count, 4)
            self.assertTrue((root / "sysroot/include/scoop_rt.h").is_file())
