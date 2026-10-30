"""Run the distribution scripts against a command recorder in an extracted layout."""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "distribution"
GNU = "x86_64-unknown-linux-gnu"
MUSL = "x86_64-unknown-linux-musl"
DARWIN = "aarch64-apple-darwin"
MSVC = "x86_64-pc-windows-msvc-ucrt"
MINGW = "x86_64-w64-windows-llvm-mingw-ucrt"

RECORDER = r"""#!/bin/sh
set -eu
printf '%s\0' "$0" "$@" >> "$SCOOP_TEST_COMMANDS"
printf '\0' >> "$SCOOP_TEST_COMMANDS"
[ "$1" != "${SCOOP_TEST_FAIL:-}" ] || exit 23
operation=$1
output=''
while [ "$#" -gt 0 ]; do
    case "$1" in -o|--output-dir) output=$2; shift ;; esac
    shift
done
case "$operation" in
    build)
        mkdir -p "$(dirname "$output")"
        if [ ! -f "$output" ]; then printf 'library' > "$output"; fi
        ;;
    doc)
        mkdir -p "$output"
        printf '<!doctype html><title>API documentation</title>' > "$output/index.html"
        ;;
esac
"""


@unittest.skipIf(os.name == "nt", "POSIX command recorder; Windows installs run in release CI")
class PostInstallTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="scoop's install ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.commands = self.root / "commands.log"
        self.environment = dict(os.environ, SCOOP_TEST_COMMANDS=str(self.commands))

    def installation(self, targets=(GNU, MUSL)):
        plan = self.root / "share/scoop/install"
        plan.mkdir(parents=True)
        (plan / "targets.list").write_text("\n".join(targets) + "\n")
        for target in targets:
            (plan / f"{target}.list").write_text("scoop.core\nscoop.std\n")
        for name in ("scoop.core", "scoop.std"):
            manifest = self.root / "src" / name / "Cone.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(f'[cone]\ngroup = "scoop"\nname = "{name}"\n')
        for name in ("post-install.sh", "post-install.ps1"):
            shutil.copy2(SCRIPTS / name, self.root / name)
        (self.root / "bin").mkdir()
        for name in ("scoop", "sco.exe"):
            recorder = self.root / "bin" / name
            recorder.write_text(RECORDER)
            recorder.chmod(0o755)
        for name in ("scoopc", "scoopc.exe"):
            (self.root / "bin" / name).touch()

    def run_install(self, *arguments, powershell=False):
        self.commands.write_bytes(b"")
        launcher = (
            ["pwsh", "-NoProfile", "-File", str(self.root / "post-install.ps1")]
            if powershell
            else ["sh", str(self.root / "post-install.sh")]
        )
        return subprocess.run(
            [*launcher, *map(str, arguments)],
            env=self.environment,
            cwd=self.root,
            capture_output=True,
            text=True,
        )

    def recorded(self):
        return [
            entry.decode().split("\0")
            for entry in self.commands.read_bytes().split(b"\0\0")
            if entry
        ]

    def test_default_build_and_docs_can_be_repeated_and_extended(self):
        self.installation()
        for arguments, target in (([], GNU), ([], GNU), (["--target", MUSL], MUSL)):
            result = self.run_install(*arguments)
            self.assertEqual(result.returncode, 0, result.stderr)
            commands = self.recorded()
            self.assertEqual([command[1] for command in commands], ["build", "build", "doc"])
            for name, command in zip(("scoop.core", "scoop.std"), commands[:2], strict=True):
                self.assertIn(str(self.root / "src" / name), command)
                self.assertIn(str(self.root / "lib" / target / (name + ".slib")), command)
                self.assertIn("--release", command)
                self.assertEqual(command[command.index("--target") + 1], target)
                self.assertIn(str(self.root / "lib" / target / "unwind"), command)
            self.assertNotIn("--unwind-prefix", commands[-1])
            for name in ("scoop.core", "scoop.std"):
                self.assertIn(str(self.root / "src" / name / "Cone.toml"), commands[-1])
            self.assertTrue((self.root / "docs" / target / "index.html").is_file())
        index = (self.root / "docs/index.html").read_text()
        self.assertIn(f"{GNU}/index.html", index)
        self.assertIn(f"{MUSL}/index.html", index)

    def test_targets_jobs_and_native_options_preserve_arguments(self):
        self.installation((DARWIN, GNU))
        cc = self.root / "compiler's path/clang"
        sdk = self.root / "native SDK"
        work = self.root / "another work directory"
        result = self.run_install(
            "--all-targets", "--cc", cc, "--native-sysroot", sdk, "--jobs", "2", "--work-dir", work
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.recorded()
        self.assertEqual(len(commands), 6)
        for command in commands:
            self.assertEqual(command[command.index("--cc") + 1], str(cc))
            self.assertEqual(command[command.index("--native-sysroot") + 1], str(sdk))
            self.assertEqual(command[command.index("--jobs") + 1], "2")
            self.assertEqual(command[command.index("--cache-dir") + 1], str(work / "cache"))
        self.assertTrue(all("--unwind-prefix" not in command for command in commands[:3]))
        result = self.run_install("--target", GNU, "--target", GNU)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.recorded()), 3)

    def test_failed_library_or_documentation_stops_the_installation(self):
        self.installation()
        for operation, count in (("build", 1), ("doc", 3)):
            with self.subTest(operation=operation):
                self.environment["SCOOP_TEST_FAIL"] = operation
                result = self.run_install()
                self.assertEqual(result.returncode, 23, result.stderr)
                self.assertEqual(len(self.recorded()), count)
                self.assertFalse((self.root / "docs/index.html").exists())

    def test_invalid_selection_is_rejected_before_building(self):
        self.installation()
        for arguments in (
            ["--target", DARWIN],
            ["--target", "../invalid"],
            ["--target", GNU, "--all-targets"],
            ["--jobs", "0"],
            ["--jobs", "bad"],
            ["--target"],
            ["--unknown"],
        ):
            with self.subTest(arguments=arguments):
                result = self.run_install(*arguments)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.recorded(), [])
        self.assertEqual(self.run_install("--help").returncode, 0)

    @unittest.skipUnless(shutil.which("pwsh"), "PowerShell is not installed")
    def test_powershell_selects_windows_tools_and_propagates_failures(self):
        self.installation((MSVC, MINGW))
        result = self.run_install("--all-targets", powershell=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.recorded()
        self.assertEqual(len(commands), 6)
        for command in commands:
            self.assertEqual(command[0], str(self.root / "bin/sco.exe"))
            target = command[command.index("--target") + 1]
            if target == MSVC:
                self.assertIn(str(self.root / "libexec/llvm/bin/clang-cl.exe"), command)
            else:
                self.assertIn(str(self.root / "lib" / target / "toolchain/bin/clang.exe"), command)
                self.assertIn(str(self.root / "lib" / target / "toolchain"), command)
        self.environment["SCOOP_TEST_FAIL"] = "build"
        result = self.run_install(powershell=True)
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertEqual(len(self.recorded()), 1)


if __name__ == "__main__":
    unittest.main()
