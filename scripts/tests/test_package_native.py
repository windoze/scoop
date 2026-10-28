"""Check the DLL layout used by Windows host commands."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_native


class WindowsDependenciesTests(unittest.TestCase):
    def test_recursive_dlls_are_beside_tools_and_external_commands(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            root, llvm, build = (work / name for name in ("installation", "llvm", "build"))
            for directory in (root / "bin", root / "commands", llvm, build):
                directory.mkdir(parents=True)
            app = build / "scoop.exe"
            app.write_bytes(b"app")
            (root / "commands/scoop-example.exe").write_bytes(b"example")
            (llvm / "runtime.dll").write_bytes(b"selected runtime")
            (llvm / "support.dll").write_bytes(b"selected support")
            (build / "runtime.dll").write_bytes(b"different runtime")
            imports = {
                "scoop.exe": ["KERNEL32.dll", "runtime.dll"],
                "scoop-example.exe": ["api-ms-win-crt-runtime-l1-1-0.dll", "runtime.dll"],
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
            self.assertEqual(calls.call_count, 6)
            for directory in (root / "bin", root / "commands"):
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


if __name__ == "__main__":
    unittest.main()
