"""Native digest normalization must preserve references and semantic differences."""

import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.assertions import process_expectations
from fixture_runner.model import AssertionFailure, ConfigurationError
from fixture_runner.native_digests import normalize_native_digests
from fixture_runner.schema import validate_step


def archive_plan(first, second, payload):
    requirement = "c" * 64
    symbol = "d" * 64
    return (
        f"native library first requirement={requirement} input={first} origins=owner\n"
        f"native library second requirement={requirement} input={second} origins=owner\n"
        f"native archive {first} members=2\n"
        f'  member 0 header=8 payload=68..168 name="a.o" digest={payload} selected=true\n'
        f'  member 1 header=168 payload=228..328 name="b.o" digest={payload} selected=false\n'
        f"native object {second} slice=0..100\n"
        f"root=_scoop$1$re${symbol}\n"
    ).encode()


class NativeDigestTests(unittest.TestCase):
    def test_archive_references_and_non_digest_fields_remain_observable(self):
        plan = archive_plan("1" * 64, "2" * 64, "3" * 64)
        normalized = normalize_native_digests(plan)
        renamed = archive_plan("4" * 64, "5" * 64, "6" * 64)
        self.assertEqual(normalized, normalize_native_digests(renamed))
        self.assertEqual(normalized, normalize_native_digests(normalized))
        mutations = [
            (b"native archive " + b"1" * 64, b"native archive " + b"2" * 64),
            (
                b"digest=" + b"3" * 64 + b" selected=false",
                b"digest=" + b"7" * 64 + b" selected=false",
            ),
            (b"header=8 ", b"header=9 "),
            (b"payload=68..168", b"payload=68..169"),
            (b"selected=false", b"selected=true"),
            (b'name="b.o"', b'name="other.o"'),
            (b"requirement=" + b"c" * 64, b"requirement=" + b"e" * 64),
            (b"origins=owner", b"origins=other"),
            (b"_scoop$1$re$" + b"d" * 64, b"_scoop$1$re$" + b"f" * 64),
        ]
        for before, after in mutations:
            with self.subTest(field=before[:35]):
                self.assertNotEqual(
                    normalized, normalize_native_digests(plan.replace(before, after))
                )

    def test_dynamic_provider_edges_retain_distinct_targets(self):
        first, second = "1" * 64, "2" * 64
        plan = (
            f'dynamic provider {first} install="libfirst" imports=1\n'
            f'  re-export "libsecond" -> {second}\n'
            f'dynamic provider {second} install="libsecond" imports=0\n'
            f'  load "libfirst" -> {first}\n'
            'dynamic binding entry -> "libfirst" source="libsecond":helper kind=Symbol weak=false\n'
        ).encode()
        renamed = plan.replace(first.encode(), b"a" * 64).replace(second.encode(), b"b" * 64)
        self.assertEqual(normalize_native_digests(plan), normalize_native_digests(renamed))
        for before, after in [
            (f'load "libfirst" -> {first}'.encode(), f'load "libfirst" -> {second}'.encode()),
            (b'install="libfirst"', b'install="libother"'),
            (b'source="libsecond":helper', b'source="libsecond":wrong'),
        ]:
            self.assertNotEqual(
                normalize_native_digests(plan),
                normalize_native_digests(plan.replace(before, after)),
            )

    def test_debug_and_display_share_ids_without_rewriting_other_ids(self):
        digest = "11" * 32
        numbers = str(list(bytes.fromhex(digest)))
        data = (
            f"native object {digest}-member-1-668 selected by helper; "
            f"NativeInputId(Digest256({numbers})); "
            f"PersistentSourceNativeExternalContractId({numbers}); unrelated={digest}"
        ).encode()
        result = normalize_native_digests(data)
        self.assertEqual(result.count(b"$NATIVE_INPUT_0"), 2)
        self.assertIn(f"PersistentSourceNativeExternalContractId({numbers})".encode(), result)
        self.assertIn(f"unrelated={digest}".encode(), result)
        self.assertIn(b"-member-1-668 selected by helper", result)
        for numbers in ([17] * 31, [17] * 33, [256] * 32):
            malformed = f"NativeInputId(Digest256({numbers}))".encode()
            self.assertEqual(normalize_native_digests(malformed), malformed)

    def test_diagnostics_keep_complete_records_and_cannot_be_updated_away(self):
        diagnostic = {
            "code": "SCOOP_LINK_FAILED",
            "message": "native object " + "1" * 64 + "-member-0-8 selected by entry",
            "notes": [],
            "origin": {"kind": "none"},
            "severity": "error",
        }
        expected = dict(
            diagnostic, message=diagnostic["message"].replace("1" * 64, "$NATIVE_INPUT_0")
        )
        step = {
            "name": "reject",
            "argv": ["scoop"],
            "exit": 1,
            "stdout": "",
            "stderr": "",
            "json": "stderr",
            "diagnostics": [expected],
            "diagnostics_normalize": ["native-digests"],
        }
        result = {
            "returncode": 1,
            "stdout": b"",
            "stderr": b"",
            "raw_stderr": b"",
            "diagnostics": [diagnostic],
        }
        validate_step(step)
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(process_expectations(step, result, {}, Path(directory)), 0)
            self.assertEqual(result["diagnostics"], [diagnostic])
            for change in [
                {"code": "OTHER_ERROR"},
                {"origin": {"kind": "tool"}},
                {"notes": ["extra"]},
            ]:
                changed = dict(result, diagnostics=[dict(diagnostic, **change)])
                with self.assertRaises(AssertionFailure):
                    process_expectations(step, changed, {}, Path(directory), True)
        for key, value in [
            ("diagnostics_normalize", ["paths"]),
            ("diagnostics_normalize", "native-digests"),
        ]:
            with self.assertRaises(ConfigurationError):
                validate_step(dict(step, **{key: value}))
        missing = copy.deepcopy(step)
        del missing["diagnostics"]
        del missing["json"]
        with self.assertRaises(ConfigurationError):
            validate_step(missing)
