"""Observe external header updates through Ninja's real compiler dependencies."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from build_support import metadata, reusable, save_record


def ninja_path(path):
    return str(path).replace("\\", "/").replace("$", "$$").replace(" ", "$ ").replace(":", "$:")


class NinjaDependencyTests(unittest.TestCase):
    def test_external_header_changes_invalidate_reuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            headers = root / "external 头文件"
            headers.mkdir()
            header = headers / "value.h"
            header.write_text("#define VALUE 42\n")
            source = root / "source.c"
            source.write_text('#include "value.h"\nint answer(void) { return VALUE; }\n')
            objects = root / "build/objects"
            objects.mkdir(parents=True)
            compiler = os.environ.get("SCOOP_TEST_CLANG", "clang").replace("\\", "/")
            (objects / "build.ninja").write_text(
                "rule cc\n"
                f'  command = "{compiler}" -MMD -MF "$out.d" -MT "$out" '
                f'-I"{headers.as_posix()}" -c "$in" -o "$out"\n'
                "  depfile = $out.d\n"
                "  deps = gcc\n"
                f"build answer.obj: cc {ninja_path(source)}\n",
                encoding="utf-8",
            )
            command = ["ninja", "-C", str(objects)]

            def build():
                result = subprocess.run(command, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

            build()
            record = root / "build/inputs.json"
            outputs = [objects / "answer.obj"]
            inputs = {"source": metadata(source)}
            save_record(record, inputs, objects)
            original = outputs[0].stat().st_mtime_ns
            self.assertTrue(reusable(record, inputs, outputs))
            self.assertEqual(outputs[0].stat().st_mtime_ns, original)
            header.write_text("#define VALUE 127\n")
            self.assertFalse(reusable(record, inputs, outputs))
            build()
            save_record(record, inputs, objects)
            self.assertTrue(reusable(record, inputs, outputs))
            self.assertTrue(source.is_file())
            self.assertTrue(header.is_file())


if __name__ == "__main__":
    unittest.main()
