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
from target_catalog import WINDOWS_TARGETS, host_definition


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
                    "sysroot/src/library/README.md": (
                        "[Contract](../../../docs/specs/LIBRARY.md#api)\n"
                        "[Build](../../../scripts/README.md)\n[Source](main.scoop)\n"
                    ),
                    "sysroot/src/library/examples/README.md": "[Build](../../../../scripts/README.md)\n",
                    "docs/specs/LIBRARY.md": "# API\n",
                    "scripts/README.md": "Build instructions\n",
                    "sysroot/src/library/Cone.toml": (
                        '[cone]\ngroup = "scoop"\nname = "library"\nversion = "0.1.0"\n'
                    ),
                    "sysroot/src/library/Cone.lock": "generated dependency resolution",
                    "sysroot/src/library/target/output": "generated output",
                    "sysroot/src/library/old.slib": "old artifact",
                    "sysroot/src/scoop.core/Cone.toml": '[cone]\ngroup = "scoop"\nname = "scoop.core"\nversion = "0.1.0"\n',
                    "scripts/distribution/README.md": "installation",
                    "scripts/distribution/RELEASE-NOTES.md": "release notes",
                    "sysroot/share/scoop/examples/catalog.toml": "schema = 1\n",
                    "sysroot/share/scoop/examples/topics/start/hello.md": "hello instructions",
                    "sysroot/share/scoop/examples/programs/start/hello/main.scoop": "fun main() {}",
                }
                for relative, content in files.items():
                    path = repository / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(content, encoding="utf-8")
                skill_path = Path("tooling/scoop-programming")
                skill_source = package_toolchain.ROOT / skill_path
                shutil.copytree(skill_source, repository / skill_path)
                for relative in (
                    "scripts/distribution/post-install.sh",
                    "scripts/distribution/post-install.ps1",
                ):
                    shutil.copy2(package_toolchain.ROOT / relative, repository / relative)
                target = host_definition(host)["default_target"]
                target_root = sysroot / "lib" / target
                target_files = [
                    "scoop.core.slib",
                    "library.slib",
                    "libscoop_text_backend.a",
                    "liburiparser.a",
                    "libpcre2-8.a",
                    *[
                        f"include/{name}"
                        for name in (
                            package_toolchain.RUST_TEXT_HEADER,
                            "uriparser/Uri.h",
                            "pcre2.h",
                            "zstd.h",
                            "zstd_errors.h",
                            "zdict.h",
                            "zlib-ng.h",
                            "zconf-ng.h",
                            "zlib_name_mangling-ng.h",
                            "brotli/decode.h",
                        )
                    ],
                    *[
                        library
                        for package in package_toolchain.COMPRESSION_PACKAGES
                        for library in package["libraries"]
                    ],
                    *[
                        f"{variant}/index.cbor"
                        for variant in (
                            "runtime",
                            "runtime-stress",
                            "runtime-startup-stats",
                            "runtime-stress-startup-stats",
                        )
                    ],
                    "openssl/include/openssl/ssl.h",
                    "openssl/lib/libssl.a",
                    "openssl/lib/libcrypto.a",
                    "nghttp2/include/nghttp2.h",
                    "nghttp2/lib/libnghttp2.a",
                ]
                if not host.endswith("darwin"):
                    target_files += ["unwind/include/unwind.h", "unwind/lib/libunwind.a"]
                if not host.endswith("linux-gnu"):
                    target_files += [
                        package_toolchain.SKIA_LIBRARY,
                        "include/skia/include/core/SkCanvas.h",
                    ]
                for relative in target_files:
                    path = target_root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(relative)
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
                    debugger_prefix=work / "lldb",
                    debugger_python=sys.executable,
                    python_license=None,
                    mingw_ucrt=work / "mingw-ucrt",
                    work_dir=work,
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
                    patch.object(
                        package_toolchain.package_debugger,
                        "stage",
                        return_value=({}, "3.14", work / "LICENSE.txt"),
                    ),
                    patch.object(package_toolchain.package_licenses, "collect"),
                ):
                    assemble(root, [target], binaries, args, release, extension)
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
                self.assertEqual(metadata["default_target"], target)
                self.assertEqual(metadata["targets"][target]["cones"], ["scoop.core", "library"])
                self.assertEqual(list(installed.rglob("*.slib")), [])
                self.assertFalse((installed / "docs").exists())
                installer = "post-install.ps1" if "windows" in host else "post-install.sh"
                self.assertTrue((installed / installer).is_file())
                self.assertEqual(
                    (installed / "share/scoop/install/targets.list").read_text().splitlines(),
                    [target],
                )
                self.assertEqual(
                    (installed / f"share/scoop/install/{target}.list").read_text().splitlines(),
                    metadata["targets"][target]["cones"],
                )
                self.assertEqual(
                    (installed / "lib" / target / "libscoop_text_backend.a").read_text(),
                    "libscoop_text_backend.a",
                )
                self.assertEqual(metadata["liburing"], "2.15")
                if not host.endswith("linux-gnu"):
                    self.assertEqual(metadata["skia"], release["skia"])
                    self.assertEqual(
                        (installed / "lib" / target / package_toolchain.SKIA_LIBRARY).read_text(),
                        package_toolchain.SKIA_LIBRARY,
                    )
                    self.assertTrue(
                        (
                            installed / "lib" / target / "include/skia/include/core/SkCanvas.h"
                        ).is_file()
                    )
                for name in ("rust_windows", "llvm_mingw"):
                    if "windows" in host:
                        self.assertEqual(metadata[name], release[name])
                    else:
                        self.assertNotIn(name, metadata)
                self.assertEqual(metadata["debugger_python"], "3.14")
                self.assertEqual(metadata["lldb"], release["llvm"])
                self.assertNotIn("libuv", metadata)
                self.assertEqual(
                    metadata["cones"],
                    [
                        {"coordinate": "scoop:library:0.1.0"},
                        {"coordinate": "scoop:scoop.core:0.1.0"},
                    ],
                )
                self.assertEqual(
                    (installed / "src/library/main.scoop").read_text(), "fun main() {}"
                )
                self.assertFalse((installed / "src/library/Cone.lock").exists())
                self.assertFalse((installed / "src/library/target").exists())
                repository_url = "https://github.com/windoze/scoop/blob/abc123/"
                self.assertEqual(
                    (installed / "src/library/README.md").read_text(),
                    f"[Contract]({repository_url}docs/specs/LIBRARY.md#api)\n"
                    f"[Build]({repository_url}scripts/README.md)\n[Source](main.scoop)\n",
                )
                self.assertEqual(
                    (installed / "src/library/examples/README.md").read_text(),
                    f"[Build]({repository_url}scripts/README.md)\n",
                )
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

    def test_packaging_prepares_only_native_resources_and_runtime(self):
        for target in ("aarch64-apple-darwin", *WINDOWS_TARGETS):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                args = SimpleNamespace(
                    work_dir=work,
                    jobs=2,
                    gn="gn",
                    llvm_prefix=work / "llvm",
                    llvm_source=work / "llvm-source",
                    mingw_ucrt=work / "ucrt",
                    mingw_msvcrt=work / "msvcrt",
                    windows_perl="perl",
                    msys_perl="perl",
                    msys_make="make",
                    msys_shell="sh",
                )
                with patch.object(package_toolchain, "run_script") as run:
                    package_toolchain.prepare_targets(args, [target], {})
                scripts = [call.args[0] for call in run.call_args_list]
                self.assertIn("build_openssl.py", scripts)
                self.assertIn("build_text_backends.py", scripts)
                self.assertIn("build_skia.py", scripts)
                self.assertEqual(scripts[-1], "prepare_sysroot.py")
                self.assertIn("--runtime-only", run.call_args.args[1])
                self.assertNotIn("--scoop", run.call_args.args[1])
                skia = next(
                    call.args[1] for call in run.call_args_list if call.args[0] == "build_skia.py"
                )
                self.assertEqual(skia[skia.index("--target") + 1], target)
                if target in WINDOWS_TARGETS:
                    self.assertEqual(
                        skia[skia.index("--cc") + 1],
                        package_toolchain.package_windows.compiler(args, target),
                    )

    def test_extract_directly_into_installation_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "scoop-1.2.3-host"
            files = {
                "VERSION": "1.2.3\nrevision: abc123\nhost: aarch64-apple-darwin\n",
                "bin/scoop": "#!/bin/sh\nexit 0\n",
                "src/scoop.core/Cone.toml": "library source manifest",
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
