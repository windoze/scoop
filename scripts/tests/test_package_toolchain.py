"""Check the installation archive layout and publication failure behavior."""

import os
import shutil
import sys
import tarfile
import tempfile
import tomllib
import unittest
from itertools import product
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_toolchain
from package_toolchain import archive, assemble


class ToolchainArchiveTests(unittest.TestCase):
    def test_tooling_and_commands_are_assembled_and_archived(self):
        release = tomllib.loads(package_toolchain.RELEASE_CONFIG.read_text())
        hosts = ("x86_64-pc-windows-gnullvm", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu")
        for host, populated in product(hosts, (False, True)):
            with (
                self.subTest(host=host, populated=populated),
                tempfile.TemporaryDirectory() as temporary,
            ):
                work = Path(temporary)
                suffix = ".exe" if "windows" in host else ""
                repository = work / "repository"
                sysroot = repository / "sysroot"
                for name in ("src", "include", "native/text_backend", "commands"):
                    (sysroot / name).mkdir(parents=True)
                (sysroot / "commands/.gitkeep").touch()
                files = {
                    "sysroot/vendor/liburing/2.15/liburing.spec": "Version: 2.15\n",
                    "sysroot/vendor/ryu/README.scoop.md": "固定 commit：`abc123`",
                    "sysroot/src/library/main.scoop": "fun main() {}",
                    "sysroot/src/library/Cone.lock": "generated dependency resolution",
                    "sysroot/src/library/target/output": "generated output",
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
                skill_path = Path("tooling/scoop-programming")
                skill_source = package_toolchain.ROOT / skill_path
                shutil.copytree(skill_source, repository / skill_path)
                if populated:
                    command = sysroot / f"commands/scoop-example{suffix}"
                    command.write_text("#!/bin/sh\nexit 37\n")
                    command.chmod(0o755)
                    (sysroot / "commands/target").mkdir()
                    (sysroot / "commands/target/data").write_text("command data")
                extension = work / "extension.vsix"
                extension.write_bytes(b"extension")
                args = SimpleNamespace(
                    version="1.2.3",
                    revision="abc123",
                    host=host,
                    llvm_prefix=work / "llvm",
                    llvm_source=work / "llvm-source",
                    prepared_sysroot=sysroot if populated else None,
                )
                if "windows" in host:
                    (args.llvm_prefix / "bin").mkdir(parents=True)
                    for name in package_toolchain.package_windows.TOOLS:
                        (args.llvm_prefix / "bin" / (name + suffix)).write_text(name)
                    header = args.llvm_prefix / "lib/clang/22/include/stddef.h"
                    header.parent.mkdir(parents=True)
                    header.write_text("Clang resource header")
                root = work / "installation"
                binaries = [work / (name + suffix) for name in package_toolchain.TOOLS]
                for binary in binaries:
                    binary.write_text(f"#!/bin/sh\necho {binary.name}\n")
                    binary.chmod(0o755)
                with (
                    patch.object(package_toolchain, "ROOT", repository),
                    patch.object(package_toolchain.package_native, "darwin", return_value=[]),
                    patch.object(package_toolchain.package_native, "linux", return_value=[]),
                    patch.object(package_toolchain.package_native, "windows", return_value=[]),
                    patch.object(package_toolchain.package_licenses, "collect"),
                ):
                    assemble(root, [], binaries, args, release, extension)
                destination = work / "toolchain.tar.xz"
                archive(root, destination)
                installed = work / "extracted"
                with tarfile.open(destination) as package:
                    package.extractall(installed, filter="data")
                driver = "sco.exe" if "windows" in host else "scoop"
                names = [
                    driver,
                    *(name + suffix for name in ("scoopc", "scoop-link", "scoop-lsp", "scoop-pm")),
                ]
                self.assertEqual({path.name for path in (installed / "bin").iterdir()}, set(names))
                for name, binary in zip(names, binaries, strict=True):
                    self.assertEqual((installed / "bin" / name).read_bytes(), binary.read_bytes())
                    if os.name != "nt":
                        self.assertTrue((installed / "bin" / name).stat().st_mode & 0o111)
                if "windows" in host:
                    self.assertEqual(
                        (installed / "libexec/llvm/bin/clang.exe").read_text(), "clang"
                    )
                    self.assertEqual(
                        (installed / "libexec/llvm/lib/clang/22/include/stddef.h").read_bytes(),
                        header.read_bytes(),
                    )
                metadata = tomllib.loads((installed / "share/scoop/toolchain.toml").read_text())
                self.assertEqual(metadata["liburing"], "2.15")
                for name in ("rust_windows", "llvm_mingw"):
                    if "windows" in host:
                        self.assertEqual(metadata[name], release[name])
                    else:
                        self.assertNotIn(name, metadata)
                self.assertNotIn("libuv", metadata)
                self.assertEqual(
                    (installed / "src/library/main.scoop").read_text(), "fun main() {}"
                )
                self.assertFalse((installed / "src/library/Cone.lock").exists())
                self.assertFalse((installed / "src/library/target").exists())
                self.assertEqual(metadata["programming_skill"], skill_path.as_posix())
                skill_files = {
                    path.relative_to(skill_source): path.read_bytes()
                    for path in skill_source.rglob("*")
                    if path.is_file()
                }
                installed_skill = installed / metadata["programming_skill"]
                self.assertEqual(
                    {
                        path.relative_to(installed_skill): path.read_bytes()
                        for path in installed_skill.rglob("*")
                        if path.is_file()
                    },
                    skill_files,
                )
                self.assertIn(Path("SKILL.md"), skill_files)
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
                    self.assertEqual((commands / command.name).read_bytes(), command.read_bytes())
                    if os.name != "nt":
                        self.assertEqual((commands / command.name).stat().st_mode & 0o777, 0o755)
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
            if os.name != "nt":
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
