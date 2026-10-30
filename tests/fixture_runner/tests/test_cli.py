"""Target selection must discover only the tools used by applicable cases."""

import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.cli import arguments, can_execute, environment, selected_target
from fixture_runner.cores import prepare_test_core
from fixture_runner.model import EnvironmentError


class EnvironmentTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        os.environ.pop("SCOOP_SYSROOT", None)
        os.environ.pop("SCOOP_CACHE_DIR", None)

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_terminal_host_is_required_only_when_selected(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            args = arguments([])
            environment(repo, work, [SimpleNamespace(data={"tools": []})], args)
            fixtures = [SimpleNamespace(data={"tools": ["terminal-host"]})]
            with self.assertRaisesRegex(EnvironmentError, "required executable is missing"):
                environment(repo, work, fixtures, args)
            binary = repo / "terminal-host"
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
            args = arguments(["--terminal-host", str(binary)])
            common = environment(repo, work, fixtures, args)
            self.assertEqual(common["terminal-host"], str(binary.resolve()))

    def test_shared_language_core_preserves_sysroot_and_reuses_unchanged_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            original = repo / "sysroot/src/scoop.core/src"
            original.mkdir(parents=True)
            (original / "core.scoop").write_text("public fun coreValue(): Int = 1\n")
            helper = repo / "tests/fixtures/language/imported-classes/support/stage3_test.scoop"
            helper.parent.mkdir(parents=True)
            helper.write_text("public fun stage3CoreAnswer(): Long = 42\n")
            prepare_test_core(repo, work, repo / "sysroot")
            shared = work / "test-core/src/stage3_test.scoop"
            stamp = shared.stat().st_mtime_ns
            prepare_test_core(repo, work, repo / "sysroot")
            self.assertEqual(shared.stat().st_mtime_ns, stamp)
            self.assertEqual(shared.read_text(), helper.read_text())
            self.assertFalse((original / "stage3_test.scoop").exists())
            self.assertEqual(
                (work / "test-core/src/core.scoop").read_text(),
                (original / "core.scoop").read_text(),
            )

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_work_directories_share_prepared_resources_and_cache(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            fixtures = [SimpleNamespace(data={"tools": []})]
            first = environment(repo, work, fixtures, arguments([]))
            marker = Path(first["cache"]) / "existing-cone.slib"
            marker.write_bytes(b"unchanged cache entry")
            second_work = work.parent / "second-work"
            second_work.mkdir()
            second = environment(repo, second_work, fixtures, arguments([]))
            self.assertEqual(first["cache"], second["cache"])
            self.assertEqual(first["sysroot"], str(repo / "sysroot"))
            self.assertEqual(first["sysroot"], second["sysroot"])
            self.assertEqual(marker.read_bytes(), b"unchanged cache entry")
            self.assertFalse((work / "sysroot").exists())
            self.assertFalse((second_work / "sysroot").exists())

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_explicit_resources_override_environment_and_configure_native_linking(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            fixtures = [SimpleNamespace(data={"tools": []})]
            os.environ["SCOOP_CACHE_DIR"] = str(repo / "env-cache")
            os.environ["SCOOP_SYSROOT"] = str(repo / "env-sysroot")
            common = environment(repo, work, fixtures, arguments([]))
            self.assertEqual(common["cache"], str(repo / "env-cache"))
            self.assertEqual(common["sysroot"], str(repo / "env-sysroot"))
            explicit = environment(
                repo,
                work,
                fixtures,
                arguments(
                    [
                        "--cache-dir",
                        str(repo / "prepared-cache"),
                        "--sysroot",
                        str(repo / "prepared-sysroot"),
                    ]
                ),
            )
            self.assertEqual(explicit["cache"], str(repo / "prepared-cache"))
            self.assertEqual(explicit["sysroot"], str(repo / "prepared-sysroot"))
            self.assertEqual(
                explicit["link_args"][-1],
                str(repo / "prepared-sysroot/lib/x86_64-unknown-linux-gnu/unwind"),
            )

    def prepare(self, directory):
        directory = Path(directory).resolve()
        repo = directory / "repo"
        work = directory / "work"
        (repo / "sysroot").mkdir(parents=True)
        (repo / "scripts").mkdir()
        (repo / "scripts/release.toml").write_text(
            '[targets.aarch64-apple-darwin]\nminimum_os = "15.4"\n'
            '[targets.x86_64-apple-darwin]\nminimum_os = "27.0.1"\n'
        )
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
                patch(
                    "fixture_runner.cli.tool_output", return_value="#define __GNUC__ 15\n"
                ) as discover,
            ):
                common = environment(
                    repo, work, fixtures, arguments(["--target", "x86_64-unknown-linux-musl"])
                )
            locate.assert_called_once_with("musl-gcc")
            discover.assert_called_once_with(
                ["/opt/musl-gcc", "-dM", "-E", "-x", "c", "-"], input=b""
            )
            self.assertEqual(common["cc"], "/opt/musl-gcc")
            self.assertEqual(common["cc_args"], ["-fPIC", "-pthread"])
            self.assertEqual(common["compile_args"], ["--target", "x86_64-unknown-linux-musl"])
            self.assertEqual(
                common["link_args"],
                common["compile_args"]
                + [
                    "--unwind-prefix",
                    str(repo / "sysroot/lib/x86_64-unknown-linux-musl/unwind"),
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
                locate.assert_called_once_with(str(Path("/opt/llvm/bin/llc")))
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
    def test_cross_compilation_does_not_discover_tools_for_skipped_execution(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            fixtures = [
                SimpleNamespace(data={"requires_target_execution": False, "tools": []}),
                SimpleNamespace(data={"tools": ["cc", "ar", "scoop"]}),
            ]
            with patch("fixture_runner.cli.tool_output") as discover:
                common = environment(
                    repo,
                    work,
                    fixtures,
                    arguments(["--target", "x86_64-unknown-linux-gnu", "--compile-only"]),
                )
                discover.assert_not_called()
            self.assertEqual(common["target"], "x86_64-unknown-linux-gnu")
            environment(repo, work, fixtures[:1], arguments(["--target", common["target"]]))

    def test_default_target_is_independent_of_distribution_order(self):
        with patch(
            "fixture_runner.cli.current_host",
            return_value={
                "default_target": "x86_64-unknown-linux-gnu",
                "distribution_targets": ["x86_64-unknown-linux-musl"],
            },
        ):
            self.assertEqual(selected_target(arguments([])), "x86_64-unknown-linux-gnu")

    @patch("fixture_runner.cli.platform.machine", return_value="arm64")
    @patch("fixture_runner.cli.platform.system", return_value="Darwin")
    def test_rosetta_execution_requires_a_successful_probe(self, *_):
        with patch("fixture_runner.cli.subprocess.run") as run:
            self.assertTrue(can_execute("aarch64-apple-darwin"))
            self.assertFalse(can_execute("x86_64-unknown-linux-gnu"))
            run.assert_not_called()
            run.return_value = SimpleNamespace(returncode=0)
            self.assertTrue(can_execute("x86_64-apple-darwin"))
            self.assertEqual(run.call_args.args[0], ["/usr/bin/arch", "-x86_64", "/usr/bin/true"])
            run.return_value = SimpleNamespace(returncode=1)
            self.assertFalse(can_execute("x86_64-apple-darwin"))
            run.side_effect = OSError("arch unavailable")
            self.assertFalse(can_execute("x86_64-apple-darwin"))

    @patch("fixture_runner.cli.platform.machine", return_value="arm64")
    @patch("fixture_runner.cli.platform.system", return_value="Darwin")
    def test_explicit_darwin_companion_uses_selected_sdk_and_compiler(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            cc, sdk = repo / "clang", repo / "MacOSX.sdk"
            with patch("fixture_runner.cli.tool_output") as discover:
                common = environment(
                    repo,
                    work,
                    [SimpleNamespace(data={"tools": ["cc"]})],
                    arguments(
                        ["--cc", str(cc), "--ar", str(repo / "ar"), "--native-sysroot", str(sdk)]
                    ),
                )
                discover.assert_not_called()
            self.assertEqual(common["cc"], str(cc))
            self.assertEqual(common["sdk"], str(sdk))
            self.assertIn(str(cc), common["compile_args"])
            self.assertIn(str(sdk), common["cc_args"])

    @patch("fixture_runner.cli.platform.machine", return_value="x86_64")
    @patch("fixture_runner.cli.platform.system", return_value="Linux")
    def test_linux_clang_companion_receives_machine_and_sysroot(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            cc, sdk = repo / "clang", repo / "native-sysroot"
            with (
                patch("fixture_runner.cli.shutil.which", return_value=str(cc)),
                patch("fixture_runner.cli.tool_output", return_value="#define __clang__ 1\n"),
            ):
                common = environment(
                    repo,
                    work,
                    [SimpleNamespace(data={"tools": ["cc"]})],
                    arguments(["--cc", str(cc), "--native-sysroot", str(sdk)]),
                )
            self.assertIn("--target=x86_64-unknown-linux-gnu", common["cc_args"])
            self.assertIn("--sysroot=" + str(sdk), common["cc_args"])

    @patch("fixture_runner.cli.platform.machine", return_value="arm64")
    @patch("fixture_runner.cli.platform.system", return_value="Darwin")
    def test_ir_companion_records_the_darwin_deployment(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            with (
                patch("fixture_runner.cli.shutil.which", return_value="/opt/llvm/bin/llc"),
                patch("fixture_runner.cli.tool_output", return_value="LLVM version 22.1.8"),
                patch.dict("os.environ", {}, clear=True),
            ):
                common = environment(
                    repo,
                    work,
                    [SimpleNamespace(data={"tools": ["llc"]})],
                    arguments(["--llc", "/opt/llvm/bin/llc"]),
                )
            self.assertEqual(common["target"], "aarch64-apple-darwin")
            self.assertEqual(common["llvm_target"], "aarch64-apple-macosx15.4")

    @patch("fixture_runner.cli.platform.machine", return_value="AMD64")
    @patch("fixture_runner.cli.platform.system", return_value="Windows")
    def test_windows_targets_keep_complete_identity_and_skip_unused_native_tools(self, *_):
        from fixture_runner.cli import WINDOWS_TARGETS

        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            native = repo / "native 雪"
            unwind = repo / "unwind"
            linker = repo / "Microsoft tools/link.exe"
            fixtures = [
                SimpleNamespace(data={"tools": []}),
                SimpleNamespace(
                    data={"targets": ["aarch64-apple-darwin"], "tools": ["cc", "ar", "llc"]}
                ),
            ]
            with patch("fixture_runner.cli.shutil.which") as locate:
                for index, target in enumerate(WINDOWS_TARGETS):
                    options = ["--native-sysroot", str(native), "--unwind-prefix", str(unwind)]
                    if not index:
                        options += ["--linker", str(linker)]
                    if index:
                        options += ["--target", target]
                    common = environment(repo, work, fixtures, arguments(options))
                    self.assertEqual(common["target"], target)
                    self.assertEqual(
                        common["compile_args"],
                        ["--target", target, "--native-sysroot", str(native)],
                    )
                    self.assertEqual(
                        common["link_args"],
                        common["compile_args"]
                        + ["--unwind-prefix", str(unwind)]
                        + (["--linker", str(linker)] if not index else []),
                    )
                    self.assertEqual(
                        common["llvm_target"],
                        "x86_64-w64-windows-gnu" if index else "x86_64-pc-windows-msvc",
                    )
                locate.assert_not_called()

    @patch("fixture_runner.cli.platform.machine", return_value="AMD64")
    @patch("fixture_runner.cli.platform.system", return_value="Windows")
    def test_windows_companions_use_selected_crt_prefix_and_explicit_microsoft_sdk(self, *_):
        from fixture_runner.cli import WINDOWS_TARGETS

        with tempfile.TemporaryDirectory() as directory:
            repo, work = self.prepare(directory)
            fixtures = [SimpleNamespace(data={"tools": ["cc", "ar"]})]
            for target in WINDOWS_TARGETS:
                with self.subTest(target=target):
                    root = repo / target
                    root.mkdir()
                    (root / "msvc.toml").write_text(
                        '[msvc]\nvc_tools="vc"\nwindows_sdk="sdk"\nsdk_version="10.0.test"\n',
                        encoding="utf-8",
                    )
                    with (
                        patch.dict(os.environ, {"LLVM_SYS_221_PREFIX": str(repo / "llvm")}),
                        patch("fixture_runner.cli.shutil.which", side_effect=lambda value: value),
                    ):
                        common = environment(
                            repo,
                            work,
                            fixtures,
                            arguments(["--target", target, "--native-sysroot", str(root)]),
                        )
                    if target == WINDOWS_TARGETS[0]:
                        self.assertEqual(common["cc"], str(repo / "llvm/bin/clang.exe"))
                        self.assertIn(str(root / "vc/include"), common["cc_args"])
                        self.assertIn(str(root / "sdk/Include/10.0.test/ucrt"), common["cc_args"])
                        self.assertIn(str(root / "sdk/Lib/10.0.test/um/x64"), common["cc_args"])
                        self.assertNotIn("-isystem", common["cc_args"])
                    else:
                        self.assertEqual(common["cc"], str(root / "bin/clang.exe"))
                        self.assertEqual(common["ar"], str(root / "bin/llvm-ar.exe"))
                        self.assertEqual(
                            common["cc_args"],
                            ["--target=x86_64-w64-windows-gnu", "--sysroot=" + str(root)],
                        )
