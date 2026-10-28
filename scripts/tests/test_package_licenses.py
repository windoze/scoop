"""Cover licenses for the standalone example reader and shared Cargo dependencies."""

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_licenses


class ExampleLicenseTests(unittest.TestCase):
    def test_both_cargo_projects_keep_shared_feature_dependencies_without_dev_crates(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = root / "repository"
            (repository / "sysroot/vendor").mkdir(parents=True)
            uring = repository / "sysroot/vendor/liburing/2.15"
            uring.mkdir(parents=True)
            (uring / "COPYING").write_text("liburing license")
            source = root / "llvm-source"
            (source / "llvm").mkdir(parents=True)
            (source / "llvm/LICENSE.TXT").write_text("LLVM license")

            def package(name, external=True):
                directory = root / "crates" / name
                directory.mkdir(parents=True, exist_ok=True)
                (directory / "LICENSE").write_text(f"{name} license")
                return {
                    "id": name,
                    "name": name,
                    "version": "1.0.0",
                    "source": "registry" if external else None,
                    "manifest_path": str(directory / "Cargo.toml"),
                    "license": "MPL-2.0" if name == "pubgrub" else "MIT",
                    "license_file": None,
                }

            def node(name, normal=(), dev=()):
                return {
                    "id": name,
                    "deps": [
                        {"pkg": target, "dep_kinds": [{"kind": kind}]}
                        for targets, kind in ((normal, None), (dev, "dev"))
                        for target in targets
                    ],
                }

            main = {
                "packages": [
                    package("scoop", False),
                    package("scoop-pm", False),
                    package("shared"),
                    package("main-only"),
                    package("pubgrub"),
                ],
                "resolve": {
                    "nodes": [
                        node("scoop", ["shared"]),
                        node("scoop-pm", ["pubgrub"]),
                        node("pubgrub"),
                        node("shared", ["main-only"]),
                        node("main-only"),
                    ]
                },
            }
            example = {
                "packages": [
                    package("scoop-example", False),
                    package("shared"),
                    package("example-only"),
                    package("test-only"),
                ],
                "resolve": {
                    "nodes": [
                        node("scoop-example", ["shared"], ["test-only"]),
                        node("shared", ["example-only"]),
                        node("example-only"),
                        node("test-only"),
                    ]
                },
            }
            installation = root / "installation"
            with patch.object(
                package_licenses.subprocess,
                "check_output",
                side_effect=[json.dumps(main), json.dumps(example)],
            ) as metadata:
                package_licenses.collect(
                    installation, repository, source, "aarch64-apple-darwin", [], []
                )
                self.assertEqual(
                    [call.kwargs["cwd"] for call in metadata.call_args_list],
                    [repository, repository / "tooling/scoop-example"],
                )
            licenses = installation / "share/licenses"
            self.assertEqual((licenses / "liburing/COPYING").read_text(), "liburing license")
            for name in ("shared", "main-only", "example-only", "pubgrub"):
                self.assertEqual(
                    (licenses / f"{name}-1.0.0/LICENSE").read_text(), f"{name} license"
                )
            self.assertIn("| pubgrub-1.0.0 | MPL-2.0 |", (licenses / "README.md").read_text())
            self.assertFalse((licenses / "test-only-1.0.0").exists())
            self.assertEqual((licenses / "README.md").read_text().count("| shared-1.0.0 |"), 1)


if __name__ == "__main__":
    unittest.main()
