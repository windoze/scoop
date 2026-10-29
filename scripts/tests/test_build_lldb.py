"""Check incremental LLDB rebuilds when an applied source patch changes."""

import os
import sys
import tempfile
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
            original = source / "lldb/changed.cpp"
            original.write_text("original source")
            unchanged = source / "lldb/unchanged.cpp"
            unchanged.write_text("unchanged source")
            for file in (original, unchanged):
                os.utime(file, ns=(1_000_000_000, 1_000_000_000))
            build_lldb.copy_sources(source, copied)
            changed = copied / "lldb/changed.cpp"
            changed.write_text("patched source")
            os.utime(changed, ns=(2_000_000_000, 2_000_000_000))
            # The compiler's dependency timestamp for the unchanged file is
            # preserved even though the original archive predates the build.
            stable = copied / "lldb/unchanged.cpp"
            build_lldb.copy_sources(source, copied)
            self.assertEqual(changed.read_text(), "original source")
            self.assertGreater(changed.stat().st_mtime_ns, 2_000_000_000)
            self.assertEqual(stable.stat().st_mtime_ns, 1_000_000_000)


if __name__ == "__main__":
    unittest.main()
