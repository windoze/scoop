"""Check host dependency discovery and the relocated installation layout."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_native


@unittest.skipUnless(sys.platform == "linux", "ELF loader and patchelf")
class LinuxDependenciesTests(unittest.TestCase):
    def test_environment_libraries_are_relocated_with_selected_llvm_dependencies(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            root, llvm, external = (work / name for name in ("installation", "llvm", "python"))
            for directory in (root / "bin", llvm, external):
                directory.mkdir(parents=True)
            compiler = os.environ.get("CC", "cc")
            support = work / "support.c"
            # LLVM's selected library must win over another copy on the inherited path.
            for directory, value in ((llvm, 42), (external, 99)):
                support.write_text(f"int support_value(void) {{ return {value}; }}\n")
                subprocess.run(
                    [
                        compiler,
                        "-shared",
                        "-fPIC",
                        str(support),
                        "-Wl,-soname,librelease_support.so.1",
                        "-o",
                        str(directory / "librelease_support.so.1"),
                    ],
                    check=True,
                )
            (llvm / "librelease_support.so").symlink_to("librelease_support.so.1")
            library = external / "librelease_python.so.1"
            source = work / "python.c"
            source.write_text(
                "int support_value(void); int python_value(void) { return support_value(); }\n"
            )
            subprocess.run(
                [
                    compiler,
                    "-shared",
                    "-fPIC",
                    str(source),
                    "-L" + str(llvm),
                    "-lrelease_support",
                    "-Wl,-soname," + library.name,
                    "-o",
                    str(library),
                ],
                check=True,
            )
            (external / "librelease_python.so").symlink_to(library.name)
            source = work / "main.c"
            source.write_text(
                "#include <stdio.h>\nint python_value(void);\n"
                'int main(void) { printf("%d\\n", python_value()); return 0; }\n'
            )
            app = work / "program"
            subprocess.run(
                [
                    compiler,
                    str(source),
                    "-L" + str(external),
                    "-lrelease_python",
                    "-Wl,-rpath-link," + str(llvm),
                    "-o",
                    str(app),
                ],
                check=True,
            )
            shutil.copy2(app, root / "bin/program")
            with patch.dict(os.environ, LD_LIBRARY_PATH=str(external)):
                libraries = package_native.linux(root, [app], llvm)
            self.assertEqual(set(libraries), {library, llvm / "librelease_support.so.1"})
            moved = work / "moved installation"
            root.rename(moved)
            result = subprocess.check_output(
                [moved / "bin/program"],
                text=True,
                env=dict(os.environ, LD_LIBRARY_PATH=""),
            )
            self.assertEqual(result, "42\n")
            self.assertTrue(library.is_file())
            self.assertTrue(app.is_file())


class WindowsDependenciesTests(unittest.TestCase):
    def test_recursive_dlls_are_beside_tools_external_commands_and_clang(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary).resolve()
            root, llvm, build = (work / name for name in ("installation", "llvm", "build"))
            for directory in (
                root / "bin",
                root / "commands",
                root / "libexec/llvm/bin",
                llvm,
                build,
            ):
                directory.mkdir(parents=True)
            app = build / "scoop.exe"
            app.write_bytes(b"app")
            (root / "commands/scoop-example.exe").write_bytes(b"example")
            (root / "libexec/llvm/bin/clang.exe").write_bytes(b"clang")
            (llvm / "runtime.dll").write_bytes(b"selected runtime")
            (llvm / "support.dll").write_bytes(b"selected support")
            (build / "runtime.dll").write_bytes(b"different runtime")
            imports = {
                "scoop.exe": ["KERNEL32.dll", "PROPSYS.dll", "runtime.dll"],
                "scoop-example.exe": ["api-ms-win-crt-runtime-l1-1-0.dll", "runtime.dll"],
                "clang.exe": ["runtime.dll"],
                "runtime.dll": ["support.dll"],
                "support.dll": ["runtime.dll", "ucrtbase.dll"],
            }

            def readobj(tool, flag, path):
                self.assertEqual(Path(tool), llvm / "llvm-readobj.exe")
                self.assertEqual(flag, "--coff-imports")
                return "\n".join(
                    "Import {\n  Name: " + name + "\n}" for name in imports[Path(path).name]
                )

            with patch.object(package_native, "output", side_effect=readobj) as calls:
                libraries = package_native.windows(root, [app], llvm)
            self.assertEqual(set(libraries), {llvm / "runtime.dll", llvm / "support.dll"})
            self.assertEqual(calls.call_count, 9)
            for directory in (root / "bin", root / "commands", root / "libexec/llvm/bin"):
                self.assertEqual((directory / "runtime.dll").read_bytes(), b"selected runtime")
                self.assertEqual((directory / "support.dll").read_bytes(), b"selected support")
                self.assertFalse((directory / "KERNEL32.dll").exists())
                self.assertFalse((directory / "ucrtbase.dll").exists())

    def test_missing_non_system_dll_reports_the_importing_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "scoop.exe"
            app.touch()
            with (
                patch.object(
                    package_native,
                    "output",
                    return_value="Import {\n  Name: missing-scoop-test.dll\n}",
                ),
                patch.object(package_native.os, "get_exec_path", return_value=[]),
                self.assertRaisesRegex(ValueError, r"missing-scoop-test.dll of .*scoop.exe"),
            ):
                package_native.windows(root, [app], root / "llvm")

    def test_vc_runtime_is_a_distributable_dependency(self):
        self.assertTrue(package_native.windows_system("KERNEL32.DLL"))
        self.assertTrue(package_native.windows_system("api-ms-win-crt-stdio-l1-1-0.dll"))
        self.assertFalse(package_native.windows_system("vcruntime140.dll"))
        self.assertFalse(package_native.windows_system("msvcp140.dll"))
        self.assertFalse(package_native.windows_system("libLLVM-22.dll"))

    def test_matching_runtime_copies_in_python_directories_can_be_packaged(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root = work / "installation"
            native = {}
            for name in ("python", "extensions"):
                source, destination = work / name, root / name
                source.mkdir()
                destination.mkdir(parents=True)
                module = source / "module.pyd"
                module.touch()
                (source / "vcruntime140.dll").write_bytes(b"same runtime")
                native[module.resolve()] = destination / module.name

            def imports(tool, flag, path):
                return "  Name: vcruntime140.dll" if Path(path).suffix == ".pyd" else ""

            with (
                patch.object(package_native, "output", side_effect=imports),
                patch.object(package_native.os, "get_exec_path", return_value=[]),
            ):
                package_native.windows(root, [], work / "llvm", native)
                for name in ("python", "extensions"):
                    self.assertEqual(
                        (root / name / "vcruntime140.dll").read_bytes(), b"same runtime"
                    )
                (work / "extensions/vcruntime140.dll").write_bytes(b"different runtime")
                with self.assertRaisesRegex(ValueError, "different host libraries"):
                    package_native.windows(root, [], work / "llvm", native)


if __name__ == "__main__":
    unittest.main()
