"""Native plan snapshots preserve selection, geometry, bindings and unknown fields."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from fixture_runner.assertions import check_all
from fixture_runner.model import AssertionFailure, ConfigurationError
from fixture_runner.native_plan import native_link_plan
from fixture_runner.schema_values import check_values

PLAN = (
    b"program-link v3\ncone scoop:scoop.core:0.1.0 kind=Library objects=100\n"
    b"link inputs: startup, cone/member order, runtime/object order, libSystem\n"
    b"native archive abc members=2\n"
    b'  member 0 header=8 payload=68..168 name="a.o" digest=one selected=true\n'
    b'  member 1 header=168 payload=228..328 name="b.o" digest=two selected=false\n'
    b'dynamic provider def install="liba" imports=1\n'
    b'  re-export "libb" -> ghi\n'
    b'dynamic binding entry -> "liba" source="libb":helper kind=Symbol weak=false\n'
    b"startup object references:\n  unrelated_core_symbol\nstartup C:\nint main() {}\n"
)


class NativePlanTests(unittest.TestCase):
    def test_unrelated_core_and_startup_do_not_change_native_expectation(self):
        changed = PLAN.replace(b"objects=100", b"objects=200").replace(
            b"unrelated_core_symbol", b"new_core_symbol"
        )
        self.assertEqual(native_link_plan(PLAN), native_link_plan(changed))
        self.assertEqual(
            native_link_plan(PLAN),
            PLAN[PLAN.index(b"link inputs:") : PLAN.index(b"startup object references:")],
        )

    def test_semantic_changes_fail_read_only_snapshot_comparison(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            golden = base / "native.plan"
            golden.write_bytes(native_link_plan(PLAN))
            mutations = [
                (b"selected=false", b"selected=true"),
                (b"payload=228..328", b"payload=228..329"),
                (b'name="b.o"', b'name="c.o"'),
                (b"digest=two", b"digest=one"),
                (b"-> ghi", b"-> def"),
                (b'"libb":helper', b'"libb":wrong'),
                (b"weak=false", b"weak=true"),
                (b"native archive", b"new-field retained\nnative archive"),
            ]
            for old, new in mutations:
                with self.subTest(field=old), self.assertRaises(AssertionFailure):
                    check_all(
                        [
                            {
                                "actual": PLAN.replace(old, new),
                                "snapshot": "native.plan",
                                "normalize": ["native-link-plan"],
                            }
                        ],
                        {},
                        base,
                        False,
                    )
                self.assertEqual(golden.read_bytes(), native_link_plan(PLAN))

    def test_missing_or_ambiguous_section_boundaries_fail(self):
        for value in [
            b"",
            PLAN.replace(b"program-link v3", b"program-link v4"),
            PLAN.replace(b"startup object references:\n", b""),
            PLAN + PLAN,
        ]:
            with self.subTest(value=value[:30]), self.assertRaises(AssertionFailure):
                native_link_plan(value)
        with self.assertRaises(ConfigurationError):
            check_values(
                {"actual": "plan", "contains": "native", "normalize": ["native-link-plan"]}
            )
