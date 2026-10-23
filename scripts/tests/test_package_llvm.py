"""Exercise local LLVM inputs, extraction reuse and archive updates."""

import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from build_llvm import VERSION
from package_llvm import prepare


class LlvmSourceTests(unittest.TestCase):
    def test_local_archive_reuse_and_update(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            original = work / f"llvm-project-{VERSION}.src"
            original.mkdir()
            license_file = original / "LICENSE.TXT"
            license_file.write_text("original license")
            archive = work / f"{original.name}.tar.xz"

            def write_archive():
                with tarfile.open(archive, "w:xz") as output:
                    output.add(original, arcname=original.name)

            write_archive()
            source, selected_archive = prepare(work / "build", None, archive)
            self.assertEqual(selected_archive, archive.resolve())
            self.assertEqual((source / "LICENSE.TXT").read_text(), "original license")
            original_stat = source.stat()
            reused, _ = prepare(work / "build", None, archive)
            self.assertEqual(reused.stat().st_mtime_ns, original_stat.st_mtime_ns)
            license_file.write_text("updated upstream license")
            write_archive()
            updated, _ = prepare(work / "build", None, archive)
            self.assertEqual((updated / "LICENSE.TXT").read_text(), "updated upstream license")
            with tarfile.open(archive, "w:xz") as output:
                output.add(original, arcname="unexpected-directory")
            with self.assertRaisesRegex(ValueError, "does not contain"):
                prepare(work / "build", None, archive)
            self.assertEqual((updated / "LICENSE.TXT").read_text(), "updated upstream license")

    def test_missing_explicit_input_is_reported(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                prepare(work, work / "missing-source", None)
            with self.assertRaises(FileNotFoundError):
                prepare(work, None, work / "missing-archive")


if __name__ == "__main__":
    unittest.main()
