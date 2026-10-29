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
                installed = moved / "libexec/lldb/python/lib/python3.14/site-packages/lldb"
                self.assertEqual((installed / "native/_lldb.abi3.so").read_text(), "native library")
                self.assertTrue((installed / "lldb-argdumper").is_file())
                self.assertFalse((installed.parent / "unrelated.py").exists())
                self.assertFalse((installed.parent.parent / "test").exists())
                result = subprocess.check_output(
                    [moved / "bin/lldb-dap", "--argument", "a $value with spaces"],
                    text=True,
                    env={"PATH": os.defpath},
                )
                self.assertIn(str(moved / "libexec/lldb/python"), result)
                self.assertIn(str(moved / "libexec/lldb/bin/lldb-server"), result)
                self.assertTrue(result.endswith("<--argument>\n<a $value with spaces>\n"))
                self.assertTrue((moved / "share/scoop/debugger/scoop_lldb.py").is_file())


if __name__ == "__main__":
    unittest.main()
