"""Target selection must discover only the tools used by applicable cases."""

import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.cli import arguments, environment
from fixture_runner.model import EnvironmentError


class EnvironmentTests(unittest.TestCase):
    def prepare(self, directory):
        repo = Path(directory) / "repo"
        work = Path(directory) / "work"
        (repo / "sysroot").mkdir(parents=True)
        work.mkdir()
        return repo, work

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_musl_companion_and_inapplicable_darwin_tools(self, *_):
        fixtures = [
            SimpleNamespace(
                data={"targets": ["aarch64-apple-darwin"], "tools": ["scoop", "ar", "llc"]}
            ),
            SimpleNamespace(data={"targets": ["x86_64-unknown-linux-musl"], "tools": ["cc"]}),
        ]
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            with (
                patch("fixture_runner.cli.shutil.which", return_value="/opt/musl-gcc") as locate,
                patch("fixture_runner.cli.tool_output") as discover,
            ):
                common = environment(
                    repo, work, fixtures, arguments(["--target", "x86_64-unknown-linux-musl"])
                )
            locate.assert_called_once_with("musl-gcc")
            discover.assert_not_called()
            self.assertEqual(common["cc"], "/opt/musl-gcc")
            self.assertEqual(common["cc_args"], ["-fPIC", "-pthread"])
            self.assertEqual(common["compile_args"], ["--target", "x86_64-unknown-linux-musl"])
            self.assertEqual(
                common["link_args"],
                common["compile_args"]
                + [
                    "--unwind-prefix",
                    str(repo / "sysroot/native/x86_64-unknown-linux-musl/unwind"),
                ],
            )
            self.assertNotIn("sdk", common)
            self.assertNotIn("ar", common)

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_ir_companion_requires_the_selected_llvm_version(self, *_):
        fixtures = [SimpleNamespace(data={"tools": ["llc"]})]
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            with (
                patch(
                    "fixture_runner.cli.shutil.which", return_value="/opt/llvm/bin/llc"
                ) as locate,
                patch(
                    "fixture_runner.cli.tool_output", return_value="LLVM version 22.1.2"
                ) as version,
            ):
                common = environment(
                    repo, work, fixtures, arguments(["--llc", "/opt/llvm/bin/llc"])
                )
                locate.assert_called_once_with("/opt/llvm/bin/llc")
                version.assert_called_once_with(["/opt/llvm/bin/llc", "--version"])
                self.assertEqual(common["llc"], "/opt/llvm/bin/llc")
                self.assertEqual(common["llvm_target"], "x86_64-unknown-linux-gnu")
                version.return_value = "LLVM version 21.1.8"
                with self.assertRaisesRegex(EnvironmentError, "require LLVM 22.1"):
                    environment(repo, work, fixtures, arguments(["--llc", "/opt/llvm/bin/llc"]))

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_missing_native_compiler_and_incompatible_execution_target(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            fixtures = [SimpleNamespace(data={"tools": ["cc"]})]
            with (
                patch("fixture_runner.cli.shutil.which", return_value=None),
                self.assertRaisesRegex(EnvironmentError, "required.*tool is missing"),
            ):
                environment(repo, work, fixtures, arguments(["--cc", "missing-gcc"]))
            with self.assertRaisesRegex(EnvironmentError, "cannot execute"):
                environment(repo, work, fixtures, arguments(["--target", "aarch64-apple-darwin"]))

    @patch("fixture_runner.cli.platform.machine", return_value="arm64")
    @patch("fixture_runner.cli.platform.system", return_value="Darwin")
    def test_ir_companion_records_the_darwin_deployment(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            with (
                patch("fixture_runner.cli.shutil.which", return_value="/opt/llvm/bin/llc"),
                patch(
                    "fixture_runner.cli.tool_output", side_effect=["LLVM version 22.1.8", "15.4"]
                ),
            ):
                common = environment(
                    repo,
                    work,
                    [SimpleNamespace(data={"tools": ["llc"]})],
                    arguments(["--llc", "/opt/llvm/bin/llc"]),
                )
            self.assertEqual(common["target"], "aarch64-apple-darwin")
            self.assertEqual(common["llvm_target"], "aarch64-apple-macosx15.4")
