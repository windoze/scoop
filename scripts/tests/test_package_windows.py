"""Build Rust host code when PATH also contains an unrelated link.exe."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import package_windows


@unittest.skipUnless(os.name == "nt", "MSVC Rust host toolchain")
class WindowsHostTests(unittest.TestCase):
    def test_cargo_build_scripts_use_the_selected_msvc_linker(self):
        if not os.environ.get("VCTOOLSINSTALLDIR"):
            self.skipTest("run from an x64 MSVC developer environment")
        with tempfile.TemporaryDirectory(prefix="rust host ") as temporary:
            work = Path(temporary)
            # Only the MSVC host linker is needed by this dependency-free crate.
            args = SimpleNamespace(
                host="x86_64-pc-windows-gnullvm", mingw_msvcrt=work, mingw_ucrt=work
            )
            wrong = work / "unrelated tools"
            wrong.mkdir()
            (wrong / "link.exe").write_bytes(b"not the Microsoft linker")
            environment = dict(os.environ, PATH=str(wrong) + os.pathsep + os.environ["PATH"])
            package_windows.host_environment(args, environment)
            (work / "Cargo.toml").write_text(
                '[package]\nname = "host-linker-probe"\nversion = "0.1.0"\nedition = "2024"\n'
                "[workspace]\n"
            )
            (work / "build.rs").write_text(
                'fn main() { println!("cargo::rustc-env=HOST_LINKER_PROBE=42"); }\n'
            )
            (work / "src").mkdir()
            (work / "src/main.rs").write_text(
                'fn main() { println!("{}", env!("HOST_LINKER_PROBE")); }\n'
            )
            result = subprocess.run(
                [
                    "cargo",
                    "run",
                    "--quiet",
                    "--offline",
                    "--target",
                    "x86_64-pc-windows-msvc",
                    "--target-dir",
                    str(work / "target"),
                ],
                cwd=work,
                env=environment,
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "42")


if __name__ == "__main__":
    unittest.main()
