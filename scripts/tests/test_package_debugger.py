"""Check debugger support files and launcher relocation with ordinary inputs."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_debugger


class DebuggerPackageTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "POSIX shell launchers")
    def test_python_native_module_helpers_and_relocated_launchers(self):
        for layout in ("lib/python3.14/site-packages", "local/lib/python3.14/dist-packages"):
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                llvm, debugger = work / "llvm", work / "lldb"
                python, stdlib = work / "python", work / "python/lib/python3.14"
                module = debugger / layout / "lldb"
                files = {
                    llvm / "lib/clang/22/include/stddef.h": "header",
                    python / "bin/python3.14": "python",
                    stdlib / "LICENSE.txt": "PSF license",
                    stdlib / "encodings/__init__.py": "encoding",
                    stdlib / "site-packages/unrelated.py": "omit",
                    stdlib / "test/test_something.py": "omit",
                    debugger / "lib/liblldb.so": "native library",
                    module / "__init__.py": "module",
                    module / "native/__init__.py": "native",
                }
                command = '#!/bin/sh\nprintf "%s\\n" "$PYTHONHOME" "$PYTHONPATH" "$LLDB_DEBUGSERVER_PATH"\nprintf "<%s>\\n" "$@"\n'
                for name in package_debugger.LLVM_TOOLS:
                    files[llvm / "bin" / name] = command
                for name in ("lldb", "lldb-dap", "lldb-argdumper", "lldb-server"):
                    files[debugger / "bin" / name] = command
                for file, content in files.items():
                    file.parent.mkdir(parents=True, exist_ok=True)
                    file.write_text(content)
                    file.chmod(0o755)
                (module / "native/_lldb.abi3.so").symlink_to(debugger / "lib/liblldb.so")
                (module / "lldb-argdumper").symlink_to(debugger / "bin/lldb-argdumper")
                root = work / "staged"
                (root / "bin").mkdir(parents=True)
                paths = {
                    "version": "3.14",
                    "stdlib": str(stdlib),
                    "prefix": str(python),
                    "executable": str(python / "bin/python3.14"),
                }
                with patch.object(package_debugger, "python_paths", return_value=paths):
                    native, version, license_ = package_debugger.stage(
                        root, llvm, debugger, "python3", "x86_64-unknown-linux-gnu"
                    )
                self.assertEqual(version, "3.14")
                self.assertEqual(license_, stdlib / "LICENSE.txt")
                self.assertEqual(
                    native[(debugger / "lib/liblldb.so").resolve()],
                    root / "libexec/llvm/lib/liblldb.so",
                )
                moved = work / "moved directory ' with spaces"
                root.rename(moved)
                self.assertEqual(
                    {entry.name for entry in (moved / "bin").iterdir()},
                    {
                        "scoop-" + name
                        for name in (*package_debugger.LLVM_TOOLS, "lldb", "lldb-dap")
                    },
                )
                installed = moved / "libexec/lldb/python/lib/python3.14/site-packages/lldb"
                self.assertEqual((installed / "native/_lldb.abi3.so").read_text(), "native library")
                self.assertTrue((installed / "lldb-argdumper").is_file())
                self.assertFalse((installed.parent / "unrelated.py").exists())
                self.assertFalse((installed.parent.parent / "test").exists())
                result = subprocess.check_output(
                    [moved / "bin/scoop-lldb-dap", "--argument", "a $value with spaces"],
                    text=True,
                    env={"PATH": os.defpath},
                )
                self.assertIn(str(moved / "libexec/lldb/python"), result)
                self.assertIn(str(moved / "libexec/lldb/bin/lldb-server"), result)
                self.assertTrue(result.endswith("<--argument>\n<a $value with spaces>\n"))
                self.assertTrue((moved / "share/scoop/debugger/scoop_lldb.py").is_file())

    @unittest.skipUnless(os.name == "nt", "Windows executable launchers")
    def test_windows_launchers_relocate_and_preserve_arguments_environment_and_exit(self):
        compiler = os.environ.get("SCOOP_TEST_MINGW_UCRT")
        if not compiler:
            self.skipTest("SCOOP_TEST_MINGW_UCRT selects the Windows host compiler")
        with tempfile.TemporaryDirectory(prefix="lldb 中文 ") as temporary:
            work = Path(temporary)
            probe_source = work / "probe.c"
            probe_source.write_text(r"""
#include <fcntl.h>
#include <io.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
int wmain(int argc, wchar_t **argv) {
    _setmode(_fileno(stdout), _O_U8TEXT);
    wprintf(L"%ls\n%ls\n%ls\n", _wgetenv(L"PYTHONHOME"), _wgetenv(L"PYTHONPATH"),
            _wgetenv(L"LLDB_DEBUGSERVER_PATH"));
    for (int index = 1; index < argc; index++) wprintf(L"<%ls>\n", argv[index]);
    return 37;
}
""")
            probe = work / "probe.exe"
            subprocess.run(
                [compiler, "-municode", "-static", str(probe_source), "-o", str(probe)], check=True
            )
            llvm, debugger, python = (work / name for name in ("llvm", "lldb", "python"))
            stdlib = python / "Lib"
            package = debugger / "Lib/site-packages/lldb"
            files = {
                llvm / "lib/clang/22/include/stddef.h": "header",
                stdlib / "LICENSE.txt": "Python license",
                stdlib / "encodings/__init__.py": "encoding",
                stdlib / "site-packages/unrelated.py": "omit",
                stdlib / "test/unrelated.py": "omit",
                python / "DLLs/_ctypes.pyd": "native Python module",
                python / "python3.dll": "Python stable ABI",
                python / "python314.dll": "Python runtime",
                package / "__init__.py": "LLDB module",
                package / "native/_lldb.pyd": "LLDB native module",
            }
            for path, content in files.items():
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            programs = [python / "python.exe"]
            programs += [llvm / "bin" / (name + ".exe") for name in package_debugger.LLVM_TOOLS]
            programs += [
                debugger / "bin" / (name + ".exe")
                for name in ("lldb", "lldb-dap", "lldb-server", "lldb-argdumper")
            ]
            for program in programs:
                program.parent.mkdir(parents=True, exist_ok=True)
                program.write_bytes(probe.read_bytes())
            paths = {
                "version": "3.14",
                "stdlib": str(stdlib),
                "prefix": str(python),
                "executable": str(python / "python.exe"),
            }
            root = work / "staged"
            with patch.object(package_debugger, "python_paths", return_value=paths):
                native, version, license_ = package_debugger.stage(
                    root,
                    llvm,
                    debugger,
                    "python",
                    "x86_64-pc-windows-gnullvm",
                    windows_compiler=compiler,
                )
            self.assertEqual(version, "3.14")
            self.assertEqual(license_, stdlib / "LICENSE.txt")
            self.assertIn((package / "native/_lldb.pyd").resolve(), native)
            moved = work / "moved 中文 directory"
            root.rename(moved)
            self.assertEqual(
                {entry.name for entry in (moved / "bin").iterdir()},
                {
                    "scoop-" + name + ".exe"
                    for name in (*package_debugger.LLVM_TOOLS, "lldb", "lldb-dap")
                },
            )
            self.assertFalse(
                (moved / "libexec/lldb/python/Lib/site-packages/unrelated.py").exists()
            )
            self.assertFalse((moved / "libexec/lldb/python/Lib/test").exists())
            self.assertTrue((moved / "share/scoop/debugger/scoop_lldb.py").is_file())
            self.assertTrue((moved / "libexec/lldb/lib/clang/22/include/stddef.h").is_file())
            arguments = ["--argument", "a $value with spaces", 'quote"trail\\', "", "中文"]
            for name in ("lldb", "lldb-dap"):
                result = subprocess.run(
                    [moved / "bin" / ("scoop-" + name + ".exe"), *arguments],
                    text=True,
                    encoding="utf-8",
                    capture_output=True,
                    env={"PATH": os.defpath, "PYTHONHOME": "wrong", "PYTHONPATH": "wrong"},
                )
                self.assertEqual(result.returncode, 37, result.stderr)
                lines = result.stdout.splitlines()
                self.assertEqual(lines[0], str(moved / "libexec/lldb/python"))
                self.assertEqual(
                    lines[1],
                    str(moved / "libexec/lldb/python/Lib/site-packages")
                    + ";"
                    + str(moved / "share/scoop/debugger"),
                )
                self.assertEqual(lines[2], str(moved / "libexec/lldb/bin/lldb-server.exe"))
                injected = ["-o", "command script import scoop_lldb"] if name == "lldb" else []
                self.assertEqual(lines[3:], [f"<{argument}>" for argument in injected + arguments])


if __name__ == "__main__":
    unittest.main()
