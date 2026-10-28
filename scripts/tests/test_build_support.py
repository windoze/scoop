"""Observe external header updates through Ninja's real compiler dependencies."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from build_support import build_lock, metadata, reusable, save_record


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


@unittest.skipUnless(os.name == "nt", "Microsoft compiler dependency format")
class MicrosoftDependencyTests(unittest.TestCase):
    def test_openssl_depfiles_track_external_header_updates(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            header = root / "external header.h"
            header.write_text("#define VALUE 42\n")
            source = root / "source.c"
            source.write_text('#include "external header.h"\nint answer(void) { return VALUE; }\n')
            objects = root / "build/objects"
            objects.mkdir(parents=True)
            output = objects / "answer.obj"
            compiler = os.environ.get("SCOOP_TEST_CLANG", "clang")
            command = [
                compiler,
                "--driver-mode=cl",
                "/nologo",
                "/showIncludes",
                "/c",
                str(source),
                "/Fo" + str(output),
            ]

            def build():
                result = subprocess.run(command, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                (objects / "answer.d").write_bytes(result.stdout)

            build()
            record = root / "build/inputs.json"
            inputs = {"source": metadata(source)}
            save_record(record, inputs, objects)
            self.assertTrue(reusable(record, inputs, [output]))
            header.write_text("#define VALUE 127\n")
            self.assertFalse(reusable(record, inputs, [output]))
            build()
            save_record(record, inputs, objects)
            self.assertTrue(reusable(record, inputs, [output]))
            self.assertTrue(header.is_file())
            self.assertTrue(source.is_file())


class BuildLockTests(unittest.TestCase):
    def test_another_process_waits_until_the_build_releases_its_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "build.lock"
            script = (
                "import sys\n"
                "from pathlib import Path\n"
                "from build_support import build_lock\n"
                "print('ready', flush=True)\n"
                "with build_lock(Path(sys.argv[1])):\n"
                "    print('acquired', flush=True)\n"
            )
            environment = dict(os.environ, PYTHONPATH=str(Path(__file__).resolve().parents[1]))
            with build_lock(path):
                process = subprocess.Popen(
                    [sys.executable, "-c", script, str(path)],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    env=environment,
                )
                self.addCleanup(process.kill)
                self.assertEqual(process.stdout.readline(), "ready\n")
                with self.assertRaises(subprocess.TimeoutExpired):
                    process.communicate(timeout=0.2)
            output, error = process.communicate(timeout=15)
            self.assertEqual(process.returncode, 0, error)
            self.assertEqual(output, "acquired\n")


if __name__ == "__main__":
    unittest.main()
