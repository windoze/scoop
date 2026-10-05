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
            SimpleNamespace(data={"targets": ["aarch64-apple-darwin"], "tools": ["scoop", "ar"]}),
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
            self.assertNotIn("sdk", common)
            self.assertNotIn("ar", common)

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
