"""Check the installation archive layout and publication failure behavior."""

import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from package_toolchain import archive


class ToolchainArchiveTests(unittest.TestCase):
    def test_extract_directly_into_installation_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "scoop-1.2.3-host"
            files = {
                "VERSION": "1.2.3\nrevision: abc123\nhost: aarch64-apple-darwin\n",
                "bin/scoop": "#!/bin/sh\nexit 0\n",
                "lib/aarch64-apple-darwin/scoop.core.slib": "library",
                "tooling/scoop-language-support-0.1.0.vsix": "extension",
            }
            for relative, content in files.items():
                path = source / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            (source / "bin/scoop").chmod(0o755)
            destination = work / "toolchain.tar.xz"
            archive(source, destination)
            installed = work / "installed home/.scoop"
            with tarfile.open(destination) as package:
                package.extractall(installed, filter="data")
            for relative, content in files.items():
                self.assertEqual((installed / relative).read_text(), content)
            self.assertTrue((installed / "bin/scoop").stat().st_mode & 0o111)
            self.assertFalse((installed / source.name).exists())

    def test_source_archive_retains_its_named_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "scoop-1.2.3-llvm-sources"
            source.mkdir()
            (source / "LICENSE.TXT").write_text("license")
            destination = work / "sources.tar.xz"
            archive(source, destination, arcname=source.name)
            with tarfile.open(destination) as package:
                self.assertEqual(package.getnames(), [source.name, source.name + "/LICENSE.TXT"])

    def test_failed_archive_keeps_previous_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "installation"
            source.mkdir()
            destination = work / "toolchain.tar.xz"
            destination.write_bytes(b"previous package")
            with (
                patch.object(tarfile.TarFile, "add", side_effect=OSError("copy failed")),
                self.assertRaisesRegex(OSError, "copy failed"),
            ):
                archive(source, destination)
            self.assertEqual(destination.read_bytes(), b"previous package")
            self.assertFalse(work.joinpath("toolchain.tar.xz.new").exists())


if __name__ == "__main__":
    unittest.main()
