"""Check the component archive layout and publication failure behavior."""

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
from build_rust_text import HEADER as RUST_TEXT_HEADER
from package_toolchain import (
    archive,
    assemble_base_libraries,
    assemble_bin,
    assemble_gui_libraries,
    assemble_llvm,
    assemble_sources,
    stage_binaries,
)
from prepare_sysroot import cone_order
from target_catalog import WINDOWS_TARGETS, host_definition


class ComponentArchiveTests(unittest.TestCase):
    def test_profiling_component_contains_both_complete_cone_sets_and_runtime(self):
        for target in ("aarch64-apple-darwin", "x86_64-pc-windows-msvc-ucrt"):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                sysroot = work / "prepared"
                source = sysroot / "lib" / target / "profiling"
                expected = set()
                cones = ["scoop.core", "scoop.window"]
                for variant in ("instrumented", "release"):
                    for cone in cones:
                        names = [f"{cone}.slib"]
                        if variant == "instrumented":
                            names.append(f"{cone}.slib.profile.json")
                        for name in names:
                            expected.add(f"{variant}/{name}")
                runtime = "runtime.lib" if target in WINDOWS_TARGETS else "runtime.a"
                for variant in (
                    "runtime",
                    "runtime-stress",
                    "runtime-startup-stats",
                    "runtime-stress-startup-stats",
                ):
                    expected.update(
                        f"{variant}/{name}" for name in (runtime, "profile.a", "index.cbor")
                    )
                for relative in [
                    *expected,
                    "runtime/unused.o",
                    "instrumented/old.profraw",
                    "libSkia.a",
                ]:
                    path = source / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(relative)
                license = work / "llvm/compiler-rt/LICENSE.TXT"
                license.parent.mkdir(parents=True)
                license.write_text("LLVM profile license")
                args = SimpleNamespace(prepared_sysroot=sysroot, llvm_source=work / "llvm")
                staged = work / "staged"
                package_toolchain.assemble_profiling_libraries(staged, target, args, cones)
                output = work / "component.tar.xz"
                archive(staged, output)
                with tarfile.open(output) as package:
                    prefix = f"lib/{target}/profiling/"
                    files = {
                        member.name.removeprefix("./")
                        for member in package.getmembers()
                        if member.isfile()
                    }
                    self.assertEqual(
                        {name.removeprefix(prefix) for name in files if name.startswith(prefix)},
                        expected,
                    )
                    self.assertIn(
                        f"share/licenses/profiling-{target}/compiler-rt/LICENSE.TXT", files
                    )
                # Missing required maps fail component assembly; normal build inputs remain.
                (source / "instrumented/scoop.core.slib.profile.json").unlink()
                with self.assertRaises(FileNotFoundError):
                    package_toolchain.assemble_profiling_libraries(
                        work / "incomplete", target, args, cones
                    )

    def test_component_packages_are_assembled_and_archived(self):
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
                        "[Peer](../../../tests/fixtures/libraries/client-network/peer.py)\n"
                        "[Vendor](../../vendor/ryu/README.scoop.md#license)\n"
                    ),
                    "sysroot/src/library/examples/README.md": (
                        "[Build](../../../../scripts/README.md)\n"
                        "[Vendor](../../../vendor/ryu/README.scoop.md)\n"
                    ),
                    "docs/specs/LIBRARY.md": "# API\n",
                    "scripts/README.md": "Build instructions\n",
                    "sysroot/src/library/Cone.toml": (
                        '[cone]\ngroup = "scoop"\nname = "library"\nversion = "0.1.0"\n'
                    ),
                    "sysroot/src/library/Cone.lock": "generated dependency resolution",
                    "sysroot/src/library/target/output": "generated output",
                    "sysroot/src/library/old.slib": "old artifact",
                    "sysroot/src/scoop.core/Cone.toml": '[cone]\ngroup = "scoop"\nname = "scoop.core"\nversion = "0.1.0"\n',
                    "sysroot/src/scoop.window/Cone.toml": (
                        '[cone]\ngroup = "scoop"\nname = "scoop.window"\nversion = "0.1.0"\n'
                    ),
                    "sysroot/src/widget/Cone.toml": (
                        '[cone]\ngroup = "scoop"\nname = "widget"\nversion = "0.1.0"\n'
                        '[dependencies]\n"scoop:library" = "0.1.0"\n'
                        '"scoop:scoop.window" = "0.1.0"\n'
                    ),
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
                    "scoop.window.slib",
                    "widget.slib",
                    "libscoop_text_backend.a",
                    "liburiparser.a",
                    "libpcre2-8.a",
                    *[
                        f"include/{name}"
                        for name in (
                            RUST_TEXT_HEADER,
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
                        f"{variant}/{name}"
                        for variant in (
                            "runtime",
                            "runtime-stress",
                            "runtime-startup-stats",
                            "runtime-stress-startup-stats",
                        )
                        for name in (
                            "index.cbor",
                            "runtime.lib" if target in WINDOWS_TARGETS else "runtime.a",
                            "objects/unneeded.o",
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
                if target in package_toolchain.SKIA_TARGETS:
                    target_files += [
                        package_toolchain.SKIA_LIBRARY,
                        "include/skia/include/core/SkCanvas.h",
                    ]
                if target == package_toolchain.GTK_TARGET:
                    target_files += ["include/gtk/gtk/gtk.h", *package_toolchain.GTK_LINK_INPUTS]
                if target == package_toolchain.TRAY_TARGET:
                    target_files += [
                        "include/tray/libdbusmenu-glib/menuitem.h",
                        *package_toolchain.TRAY_LINK_INPUTS,
                    ]
                for relative in target_files:
                    path = target_root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(relative)
                if populated:
                    for name in ("scoop-example", "scoop-bundle", "scoop-api", "scoop-pgo"):
                        command = sysroot / f"commands/{name}{suffix}"
                        command.write_text("#!/bin/sh\nexit 37\n")
                        command.chmod(0o755)
                    (sysroot / "commands/target").mkdir()
                    (sysroot / "commands/target/data").write_text("command data")
                extension = work / "extension.vsix"
                extension.write_bytes(b"extension")
                llvm_prefix = work / "llvm-prefix"
                (llvm_prefix / "bin").mkdir(parents=True)
                for name in (
                    *package_toolchain.package_llvm.TOOLS,
                    *package_toolchain.package_llvm.WINDOWS_TOOLS,
                ):
                    (llvm_prefix / "bin" / (name + suffix)).write_text(name)
                (llvm_prefix / "bin" / f"llvm-config{suffix}").write_text("llvm-config")
                resource = llvm_prefix / "lib/clang/22/include/stddef.h"
                resource.parent.mkdir(parents=True)
                resource.write_text("Clang resource header")
                dylib = llvm_prefix / "lib" / "libLLVM.dylib"
                dylib.write_text("LLVM library")
                if os.name != "nt":
                    (llvm_prefix / "lib" / "libLLVM-22.dylib").symlink_to("libLLVM.dylib")
                (llvm_prefix / "include/llvm-c").mkdir(parents=True)
                (llvm_prefix / "include/llvm-c/Core.h").write_text("LLVM development header")
                args = SimpleNamespace(
                    version="1.2.3",
                    revision="abc123",
                    host=host,
                    llvm_prefix=llvm_prefix,
                    llvm_source=work / "llvm-source",
                    prepared_sysroot=sysroot if populated else None,
                    debugger_prefix=work / "lldb",
                    debugger_python=sys.executable,
                    python_license=None,
                    mingw_ucrt=work / "mingw-ucrt",
                    work_dir=work,
                )
                binaries = [work / (name + suffix) for name in package_toolchain.TOOLS]
                for binary in binaries:
                    binary.write_text(f"#!/bin/sh\necho {binary.name}\n")
                    binary.chmod(0o755)
                selection = work / "cones" / target
                selection.mkdir(parents=True)
                orders = {target: cone_order(sysroot, selection, target)}
                host_system = host_definition(host)["system"]
                roots = {
                    name: work / "roots" / name
                    for name in ("bin", "sources", "lib-base", "lib-gui")
                }
                with (
                    patch.object(package_toolchain, "ROOT", repository),
                    patch.object(
                        package_toolchain.package_debugger,
                        "stage",
                        return_value=({}, "3.14", work / "LICENSE.txt", []),
                    ),
                    patch.object(package_toolchain.package_licenses, "collect_bin"),
                    patch.object(package_toolchain.package_licenses, "collect_llvm"),
                    patch.object(package_toolchain.package_licenses, "collect_base_libraries"),
                    patch.object(package_toolchain.package_licenses, "collect_gui_libraries"),
                ):
                    stage_binaries(roots["bin"], binaries, host_system)
                    assemble_llvm(roots["bin"], args, host_system)
                    package_toolchain.stage_commands(roots["bin"], args)
                    assemble_bin(
                        roots["bin"], [target], args, release, extension, orders, "3.14", []
                    )
                    assemble_sources(roots["sources"], args)
                    base_cones, gui_cones = package_toolchain.split_gui_cones(
                        sysroot, orders[target]
                    )
                    self.assertEqual(base_cones, ["scoop.core", "library"])
                    self.assertEqual(gui_cones, ["scoop.window", "widget"])
                    assemble_base_libraries(roots["lib-base"], target, args, base_cones)
                    assemble_gui_libraries(roots["lib-gui"], target, args, gui_cones)
                output = work / "dist" / f"scoop-{args.version}"
                output.mkdir(parents=True)
                names = {
                    "bin": f"scoop-1.2.3-bin-{host}",
                    "sources": "scoop-1.2.3-sources",
                    "lib-base": f"scoop-1.2.3-lib-base-{target}",
                    "lib-gui": f"scoop-1.2.3-lib-gui-{target}",
                }
                installed = {}
                for component, root in roots.items():
                    destination = output / f"{names[component]}.tar.xz"
                    archive(root, destination)
                    with tarfile.open(destination) as package:
                        members = package.getnames()
                        self.assertFalse(any("/docs/" in name for name in members))
                        if component == "lib-base":
                            self.assertFalse(any(name.endswith(".h") for name in members))
                            self.assertIn(f"./lib/{target}/scoop.core.slib", members)
                            self.assertIn(f"./lib/{target}/library.slib", members)
                            self.assertNotIn(f"./lib/{target}/widget.slib", members)
                        elif component == "lib-gui":
                            self.assertFalse(any(name.endswith(".h") for name in members))
                            self.assertEqual(
                                sorted(name for name in members if name.endswith(".slib")),
                                [
                                    f"./lib/{target}/scoop.window.slib",
                                    f"./lib/{target}/widget.slib",
                                ],
                            )
                        else:
                            self.assertFalse(any(name.endswith(".slib") for name in members))
                        extracted = work / "extracted" / component
                        package.extractall(extracted, filter="data")
                        installed[component] = extracted

                bin_root = installed["bin"]
                driver = "sco.exe" if "windows" in host else "scoop"
                names_in_bin = [
                    driver,
                    *(name + suffix for name in ("scoopc", "scoop-link", "scoop-lsp", "scoop-pm")),
                ]
                self.assertEqual(
                    {path.name for path in (bin_root / "bin").iterdir()}, set(names_in_bin)
                )
                for name, binary in zip(names_in_bin, binaries, strict=True):
                    self.assertEqual((bin_root / "bin" / name).read_bytes(), binary.read_bytes())
                    if os.name != "nt":
                        self.assertTrue((bin_root / "bin" / name).stat().st_mode & 0o111)
                self.assertEqual(
                    (bin_root / "VERSION").read_text(),
                    f"1.2.3\nrevision: abc123\nhost: {host}\n",
                )
                metadata = tomllib.loads((bin_root / "share/scoop/toolchain.toml").read_text())
                self.assertEqual(metadata["default_target"], target)
                self.assertEqual(metadata["targets"][target]["cones"], ["scoop.core", "library"])
                self.assertEqual(
                    metadata["targets"][target]["gui_cones"], ["scoop.window", "widget"]
                )
                installer = "post-install.ps1" if "windows" in host else "post-install.sh"
                self.assertTrue((bin_root / installer).is_file())
                self.assertEqual(
                    (bin_root / "share/scoop/install/targets.list").read_text().splitlines(),
                    [target],
                )
                self.assertEqual(
                    (bin_root / f"share/scoop/install/{target}.list").read_text().splitlines(),
                    metadata["targets"][target]["cones"],
                )
                self.assertEqual(
                    (bin_root / f"share/scoop/install/{target}.gui.list").read_text().splitlines(),
                    metadata["targets"][target]["gui_cones"],
                )
                self.assertEqual(metadata["liburing"], "2.15")
                if target in package_toolchain.SKIA_TARGETS:
                    self.assertEqual(metadata["skia"], release["skia"])
                for name in ("rust_windows", "llvm_mingw"):
                    if "windows" in host:
                        self.assertEqual(metadata[name], release[name])
                    else:
                        self.assertNotIn(name, metadata)
                self.assertEqual(metadata["debugger_python"], "3.14")
                self.assertEqual(metadata["lldb"], release["llvm"])
                self.assertEqual(metadata["host_libraries"], [])
                self.assertNotIn("libuv", metadata)
                self.assertEqual(
                    metadata["cones"],
                    [
                        {"coordinate": "scoop:library:0.1.0"},
                        {"coordinate": "scoop:scoop.core:0.1.0"},
                        {"coordinate": "scoop:scoop.window:0.1.0"},
                        {"coordinate": "scoop:widget:0.1.0"},
                    ],
                )
                self.assertEqual(metadata["programming_skill"], skill_path.as_posix())
                skill_files = {
                    path.relative_to(skill_source): path.read_bytes()
                    for path in skill_source.rglob("*")
                    if path.is_file()
                }
                installed_skill = bin_root / metadata["programming_skill"]
                self.assertEqual(
                    {
                        path.relative_to(installed_skill): path.read_bytes()
                        for path in installed_skill.rglob("*")
                        if path.is_file()
                    },
                    skill_files,
                )
                self.assertIn(Path("SKILL.md"), skill_files)
                commands = bin_root / "commands"
                self.assertTrue(commands.is_dir())
                self.assertFalse((commands / ".gitkeep").exists())
                if populated:
                    for name in ("scoop-example", "scoop-bundle", "scoop-api", "scoop-pgo"):
                        command = sysroot / f"commands/{name}{suffix}"
                        self.assertEqual(
                            (commands / command.name).read_bytes(), command.read_bytes()
                        )
                        if os.name != "nt":
                            self.assertEqual(
                                (commands / command.name).stat().st_mode & 0o777, 0o755
                            )
                    self.assertEqual((commands / "target/data").read_text(), "command data")
                else:
                    self.assertEqual(list(commands.iterdir()), [])

                llvm_root = installed["bin"]
                self.assertEqual(
                    (llvm_root / "libexec/llvm/bin" / f"clang{suffix}").read_text(), "clang"
                )
                self.assertEqual(
                    (llvm_root / "libexec/llvm/lib/clang/22/include/stddef.h").read_bytes(),
                    resource.read_bytes(),
                )
                self.assertFalse((llvm_root / "libexec/llvm/include").exists())
                self.assertFalse((llvm_root / "libexec/llvm/bin" / f"llvm-config{suffix}").exists())
                self.assertFalse((llvm_root / "libexec/llvm/lib/libLLVM.dylib").exists())

                sources_root = installed["sources"]
                self.assertEqual(
                    (sources_root / "src/library/main.scoop").read_text(), "fun main() {}"
                )
                self.assertFalse((sources_root / "src/library/Cone.lock").exists())
                self.assertFalse((sources_root / "src/library/target").exists())
                repository_url = "https://github.com/scoop-lang/scoop/blob/abc123/"
                self.assertEqual(
                    (sources_root / "src/library/README.md").read_text(),
                    f"[Contract]({repository_url}docs/specs/LIBRARY.md#api)\n"
                    f"[Build]({repository_url}scripts/README.md)\n[Source](main.scoop)\n"
                    f"[Peer]({repository_url}tests/fixtures/libraries/client-network/peer.py)\n"
                    f"[Vendor]({repository_url}sysroot/vendor/ryu/README.scoop.md#license)\n",
                )
                self.assertEqual(
                    (sources_root / "src/library/examples/README.md").read_text(),
                    f"[Build]({repository_url}scripts/README.md)\n"
                    f"[Vendor]({repository_url}sysroot/vendor/ryu/README.scoop.md)\n",
                )
                for relative in (
                    "catalog.toml",
                    "topics/start/hello.md",
                    "programs/start/hello/main.scoop",
                ):
                    self.assertEqual(
                        (sources_root / "share/scoop/examples" / relative).read_bytes(),
                        (sysroot / "share/scoop/examples" / relative).read_bytes(),
                    )

                lib_root = installed["lib-base"] / "lib" / target
                gui_root = installed["lib-gui"] / "lib" / target
                self.assertEqual((lib_root / "scoop.core.slib").read_text(), "scoop.core.slib")
                self.assertEqual((lib_root / "library.slib").read_text(), "library.slib")
                self.assertEqual((gui_root / "widget.slib").read_text(), "widget.slib")
                self.assertEqual(
                    (lib_root / "libscoop_text_backend.a").read_text(), "libscoop_text_backend.a"
                )
                self.assertFalse((lib_root / package_toolchain.SKIA_LIBRARY).exists())
                self.assertFalse((lib_root / "include/skia").exists())
                self.assertFalse((gui_root / "runtime").exists())
                if target in package_toolchain.SKIA_TARGETS:
                    self.assertEqual(
                        (gui_root / package_toolchain.SKIA_LIBRARY).read_text(),
                        package_toolchain.SKIA_LIBRARY,
                    )
                    self.assertFalse((gui_root / "include").exists())
                self.assertFalse((lib_root / "include/gtk").exists())
                if target == package_toolchain.GTK_TARGET:
                    self.assertFalse((gui_root / "include").exists())
                    for library in package_toolchain.GTK_LINK_INPUTS:
                        self.assertEqual((gui_root / library).read_text(), library)
                        self.assertFalse((lib_root / library).exists())
                else:
                    self.assertFalse((gui_root / "include/gtk").exists())
                self.assertFalse((lib_root / "include/tray").exists())
                if target == package_toolchain.TRAY_TARGET:
                    self.assertFalse(
                        (gui_root / "include/tray/libdbusmenu-glib/menuitem.h").is_file()
                    )
                    for library in package_toolchain.TRAY_LINK_INPUTS:
                        self.assertEqual((gui_root / library).read_text(), library)
                else:
                    self.assertFalse((gui_root / "include/tray").exists())
                for variant in (
                    "runtime",
                    "runtime-stress",
                    "runtime-startup-stats",
                    "runtime-stress-startup-stats",
                ):
                    self.assertTrue((lib_root / variant / "index.cbor").is_file())
                    runtime_archive = "runtime.lib" if target in WINDOWS_TARGETS else "runtime.a"
                    self.assertTrue((lib_root / variant / runtime_archive).is_file())
                    self.assertFalse((lib_root / variant / "objects").exists())

    def test_packaging_builds_system_libraries_and_runtime(self):
        for target in (
            "aarch64-apple-darwin",
            *WINDOWS_TARGETS,
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
        ):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                args = SimpleNamespace(
                    work_dir=work,
                    jobs=2,
                    gn="gn",
                    llvm_prefix=work / "llvm",
                    gtk_prefix=work / "gtk",
                    tray_prefix=work / "tray",
                    llvm_source=work / "llvm-source",
                    mingw_ucrt=work / "ucrt",
                    mingw_msvcrt=work / "msvcrt",
                    windows_perl="perl",
                    msys_perl="perl",
                    msys_make="make",
                    msys_shell="sh",
                    gnu_cc="gcc",
                    musl_cc="musl-gcc",
                    unwind_cc="clang",
                    unwind_cxx="clang++",
                    musl_include=work / "musl/include",
                )
                binaries = [work / "scoop", work / "scoopc"]
                with patch.object(package_toolchain, "run_script") as run:
                    package_toolchain.prepare_targets(args, [target], binaries, {})
                scripts = [call.args[0] for call in run.call_args_list]
                self.assertIn("build_openssl.py", scripts)
                self.assertIn("build_text_backends.py", scripts)
                self.assertIn("build_profile_runtime.py", scripts)
                self.assertEqual(scripts.count("prepare_sysroot.py"), 2)
                self.assertEqual(scripts[-1], "prepare_sysroot.py")
                arguments = run.call_args.args[1]
                self.assertIn("--profiling", arguments)
                self.assertIn("--profile-runtime", arguments)
                self.assertNotIn("--runtime-only", arguments)
                self.assertEqual(arguments[arguments.index("--scoop") + 1], binaries[0])
                self.assertEqual(
                    "prepare_gtk.py" in scripts, target == package_toolchain.GTK_TARGET
                )
                if target == package_toolchain.GTK_TARGET:
                    gtk = next(
                        call.args[1]
                        for call in run.call_args_list
                        if call.args[0] == "prepare_gtk.py"
                    )
                    self.assertEqual(gtk[gtk.index("--development-prefix") + 1], args.gtk_prefix)
                self.assertEqual(
                    "prepare_tray.py" in scripts, target == package_toolchain.TRAY_TARGET
                )
                if target == package_toolchain.TRAY_TARGET:
                    tray = next(
                        call.args[1]
                        for call in run.call_args_list
                        if call.args[0] == "prepare_tray.py"
                    )
                    self.assertEqual(tray[tray.index("--development-prefix") + 1], args.tray_prefix)
                if target == "x86_64-unknown-linux-musl":
                    self.assertNotIn("build_skia.py", scripts)
                    continue
                self.assertIn("build_skia.py", scripts)
                skia = next(
                    call.args[1] for call in run.call_args_list if call.args[0] == "build_skia.py"
                )
                self.assertEqual(skia[skia.index("--target") + 1], target)
                if target in WINDOWS_TARGETS:
                    self.assertEqual(
                        skia[skia.index("--cc") + 1],
                        package_toolchain.package_windows.compiler(args, target),
                    )
                elif target == "x86_64-unknown-linux-gnu":
                    self.assertEqual(skia[skia.index("--cc") + 1], args.llvm_prefix / "bin/clang")

    def test_extract_directly_into_installation_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            source = work / "scoop-1.2.3-bin-host"
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
