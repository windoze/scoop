"""Check incremental LLDB rebuilds when an applied source patch changes."""

import os
import platform
import sys
import sysconfig
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import build_lldb


class LldbBuildTests(unittest.TestCase):
    def test_reverted_patch_updates_only_affected_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "original"
            copied = Path(temporary) / "copied"
            for component in build_lldb.SOURCE_PARTS:
                (source / component).mkdir(parents=True, exist_ok=True)
            for component in build_lldb.SOURCE_FILES:
                (source / component).write_text("build configuration")
            original = source / "lldb/source/changed.cpp"
            original.write_text("original source")
            unchanged = source / "lldb/source/unchanged.cpp"
            unchanged.write_text("unchanged source")
            for file in (original, unchanged):
                os.utime(file, ns=(1_000_000_000, 1_000_000_000))
            build_lldb.copy_sources(source, copied)
            changed = copied / "lldb/source/changed.cpp"
            changed.write_text("patched source")
            os.utime(changed, ns=(2_000_000_000, 2_000_000_000))
            # The compiler's dependency timestamp for the unchanged file is
            # preserved even though the original archive predates the build.
            stable = copied / "lldb/source/unchanged.cpp"
            build_lldb.copy_sources(source, copied)
            self.assertEqual(changed.read_text(), "original source")
            self.assertGreater(changed.stat().st_mtime_ns, 2_000_000_000)
            self.assertEqual(stable.stat().st_mtime_ns, 1_000_000_000)

    @unittest.skipUnless(platform.system() == "Linux", "Linux loader paths")
    def test_installed_rpath_locates_the_selected_python_library(self):
        host = build_lldb.current_host(platform.system(), platform.machine())["rust_target"]
        release = tomllib.loads(build_lldb.RELEASE_CONFIG.read_text())
        llvm = Path("/opt/scoop-llvm")
        options = build_lldb.configure(
            Path("/source"),
            Path("/build"),
            Path("/prefix"),
            llvm,
            sys.executable,
            host,
            release,
            "cc",
            "c++",
        )
        rpath = next(value for value in options if value.startswith("-DCMAKE_INSTALL_RPATH="))
        self.assertEqual(
            rpath.split("=", 1)[1].split(";"),
            ["$ORIGIN/../lib", str(llvm / "lib"), sysconfig.get_config_var("LIBDIR")],
        )


if __name__ == "__main__":
    unittest.main()
