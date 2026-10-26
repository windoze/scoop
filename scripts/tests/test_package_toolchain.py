"""Check the installation archive layout and publication failure behavior."""

import sys
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_toolchain
from package_toolchain import archive, assemble


class ToolchainArchiveTests(unittest.TestCase):
    def test_commands_are_assembled_and_archived_with_executable_permissions(self):
        release = tomllib.loads(package_toolchain.RELEASE_CONFIG.read_text())
        for populated in (False, True):
            with self.subTest(populated=populated), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                repository = work / "repository"
                sysroot = repository / "sysroot"
                for name in ("src", "include", "native/text_backend", "commands"):
                    (sysroot / name).mkdir(parents=True)
                (sysroot / "commands/.gitkeep").touch()
                files = {
                    "sysroot/vendor/libuv/include/uv/version.h": "\n".join(
                        f"#define UV_VERSION_{part} {number}"
                        for part, number in (("MAJOR", 1), ("MINOR", 0), ("PATCH", 0))
                    ),
                    "sysroot/vendor/ryu/README.scoop.md": "固定 commit：`abc123`",
                    "scripts/distribution/README.md": "installation",
                    "scripts/distribution/RELEASE-NOTES.md": "release notes",
                    "sysroot/share/scoop/examples/catalog.toml": "schema = 1\n",
                    "sysroot/share/scoop/examples/topics/start/hello.md": "hello instructions",
                    "sysroot/share/scoop/examples/programs/start/hello/main.scoop": "fun main() {}",
                }
                for relative, content in files.items():
                    path = repository / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(content)
                if populated:
                    command = sysroot / "commands/scoop-example"
                    command.write_text("#!/bin/sh\nexit 37\n")
                    command.chmod(0o755)
                    (sysroot / "commands/target").mkdir()
                    (sysroot / "commands/target/data").write_text("command data")
                extension = work / "extension.vsix"
                extension.write_bytes(b"extension")
                args = SimpleNamespace(
                    version="1.2.3",
                    revision="abc123",
                    host="aarch64-apple-darwin",
                    llvm_prefix=work / "llvm",
                    llvm_source=work / "llvm-source",
                )
                root = work / "installation"
                with (
                    patch.object(package_toolchain, "ROOT", repository),
                    patch.object(package_toolchain.package_native, "darwin", return_value=[]),
                    patch.object(package_toolchain.package_licenses, "collect"),
                ):
                    assemble(root, [], [], args, release, extension)
                destination = work / "toolchain.tar.xz"
                archive(root, destination)
                installed = work / "extracted"
                with tarfile.open(destination) as package:
                    package.extractall(installed, filter="data")
                commands = installed / "commands"
                for relative in (
                    "catalog.toml",
                    "topics/start/hello.md",
                    "programs/start/hello/main.scoop",
                ):
                    self.assertEqual(
                        (installed / "share/scoop/examples" / relative).read_bytes(),
                        (sysroot / "share/scoop/examples" / relative).read_bytes(),
                    )
                self.assertTrue(commands.is_dir())
                self.assertFalse((commands / ".gitkeep").exists())
                if populated:
                    self.assertEqual(
                        (commands / "scoop-example").read_bytes(), command.read_bytes()
                    )
                    self.assertEqual((commands / "scoop-example").stat().st_mode & 0o777, 0o755)
                    self.assertEqual((commands / "target/data").read_text(), "command data")
                else:
                    self.assertEqual(list(commands.iterdir()), [])

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
