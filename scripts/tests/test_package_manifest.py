"""Exercise release selection and merging independently produced host fragments."""

import hashlib
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_manifest
from build_support import RELEASE_CONFIG
from target_catalog import host_definition


class ManifestTests(unittest.TestCase):
    def produce(self, root, host):
        root.mkdir()
        targets = list(host_definition(host)["distribution_targets"])
        components = ["sources", "vendor", "bin-" + host]
        for target in targets:
            components.append("lib-base-" + target)
            components.append("lib-profiling-" + target)
            if "musl" not in target:
                components.append("lib-gui-" + target)
            if "llvm-mingw" in target:
                components.append("native-" + target)
        names = []
        for component in components:
            name = f"scoop-1.2.3-{component}.tar.xz"
            (root / name).write_bytes(component.encode())
            names.append(name)
        (root / "scoop-1.2.3-llvm-sources.tar.xz").write_bytes(b"retired archive")
        return package_manifest.produce(
            root,
            "1.2.3",
            "revision",
            host,
            targets,
            names,
            tomllib.loads(RELEASE_CONFIG.read_text()),
        )

    def test_three_hosts_share_one_source_selection_and_keep_target_dependencies(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            hosts = (
                "aarch64-apple-darwin",
                "x86_64-unknown-linux-gnu",
                "x86_64-pc-windows-gnullvm",
            )
            fragments = []
            for host in hosts:
                directory = root / host
                self.produce(directory, host)
                fragments.append(directory / f"manifest-{host}.toml")
                self.assertNotIn("sources", tomllib.loads(fragments[-1].read_text())["archives"])
            fragments.append(root / hosts[0] / "manifest-shared.toml")
            merged = package_manifest.merge(fragments)
            package_manifest.write(root / "manifest.toml", merged)
            actual = tomllib.loads((root / "manifest.toml").read_text())
            self.assertEqual(set(actual["hosts"]), set(hosts))
            self.assertEqual(len(actual["targets"]), 7)
            self.assertNotIn("gui", actual["targets"]["x86_64-unknown-linux-musl"])
            for target, selection in actual["targets"].items():
                self.assertEqual(selection["profiling"], "lib-profiling-" + target)
            for crt in ("ucrt", "msvcrt"):
                target = f"x86_64-w64-windows-llvm-mingw-{crt}"
                self.assertEqual(actual["targets"][target]["native"], "native-" + target)
            self.assertNotIn("llvm-sources", actual["archives"])
            self.assertEqual(actual["archives"]["sources"]["size"], len(b"sources"))
            self.assertEqual(
                actual["archives"]["sources"]["sha256"], hashlib.sha256(b"sources").hexdigest()
            )

    def test_missing_component_and_conflicting_release_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            host = "aarch64-apple-darwin"
            manifest = self.produce(root / "host", host)
            del manifest["archives"]["lib-base-aarch64-apple-darwin"]
            with self.assertRaisesRegex(ValueError, "Missing component"):
                package_manifest.validate(manifest)
            shared = root / "host/manifest-shared.toml"
            content = tomllib.loads(shared.read_text())
            content["revision"] = "another revision"
            package_manifest.write(shared, content)
            with self.assertRaisesRegex(ValueError, "Release identity differs"):
                package_manifest.merge([root / "host" / f"manifest-{host}.toml", shared])

    def test_profiling_reference_requires_archive_but_is_optional(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = self.produce(Path(temporary) / "host", "aarch64-apple-darwin")
            target = "aarch64-apple-darwin"
            del manifest["archives"][manifest["targets"][target]["profiling"]]
            with self.assertRaisesRegex(ValueError, "Missing component.*lib-profiling"):
                package_manifest.validate(manifest)
            del manifest["targets"][target]["profiling"]
            package_manifest.validate(manifest)


if __name__ == "__main__":
    unittest.main()
