"""Exercise actual child processes, paired runs and unsuccessful comparisons."""

import json
import os
import statistics
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

BENCHMARKS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCHMARKS))

import compare  # noqa: E402
from process_metrics import gc_statistics, run_process  # noqa: E402


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        temporary = BENCHMARKS.parents[1] / "tmp"
        temporary.mkdir(exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(prefix="compare-test-", dir=temporary)
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def options(self, code, *extra):
        return compare.arguments(
            [
                "--baseline",
                sys.executable,
                "--candidate",
                sys.executable,
                "--work",
                str(self.root / "results"),
                "--runs=2",
                "--warmups=1",
                "--arg=-c",
                f"--arg={code}",
                *extra,
            ]
        )

    def test_child_output_and_resources(self):
        stdout, stderr = self.root / "stdout", self.root / "stderr"
        result = run_process(
            [
                sys.executable,
                "-c",
                "import os; os.write(1, b'a' * 200000); os.write(2, b'b' * 200000)",
            ],
            dict(os.environ),
            stdout,
            stderr,
        )
        self.assertEqual(stdout.read_bytes(), b"a" * 200000)
        self.assertEqual(stderr.read_bytes(), b"b" * 200000)
        self.assertGreater(result["wall_seconds"], 0)
        self.assertGreater(result["peak_rss_bytes"], 0)
        self.assertGreaterEqual(result["cpu_seconds"], 0)

    def test_alternating_pairs_and_excluded_warmups(self):
        trace = self.root / "order"
        code = (
            "import os; from pathlib import Path; "
            f"p = Path({str(trace)!r}); "
            "p.open('a').write(os.environ['VARIANT'] + '\\n'); print('answer')"
        )
        report = compare.compare(
            self.options(code, "--baseline-env=VARIANT=A", "--candidate-env=VARIANT=B")
        )
        self.assertEqual(trace.read_text().splitlines(), ["A", "B", "A", "B", "B", "A"])
        self.assertEqual(len(report["warmup_samples"]), 2)
        self.assertEqual(len(report["samples"]), 4)
        baseline = [s["wall_seconds"] for s in report["samples"] if s["variant"] == "baseline"]
        self.assertEqual(
            report["summary"]["wall_seconds"]["baseline_median"], statistics.median(baseline)
        )
        self.assertEqual(len(report["summary"]["paired_wall_ratios"]), 2)
        self.assertEqual(json.loads((self.root / "results/report.json").read_text()), report)

    def test_failed_child_keeps_output_without_success_report(self):
        options = self.options("import sys; print('failure', file=sys.stderr); sys.exit(7)")
        with self.assertRaisesRegex(RuntimeError, "exit 7"):
            compare.compare(options)
        work = self.root / "results"
        self.assertIn("failure", (work / "warmup-000-baseline.stderr").read_text())
        self.assertFalse((work / "report.json").exists())

    def test_output_difference_is_rejected(self):
        options = self.options(
            "import os; print(os.environ['ANSWER'])",
            "--baseline-env=ANSWER=right",
            "--candidate-env=ANSWER=wrong",
        )
        with self.assertRaisesRegex(RuntimeError, "candidate: stdout differs"):
            compare.compare(options)
        self.assertFalse((self.root / "results/report.json").exists())

    def test_expected_output_rejects_identical_wrong_answers(self):
        expected = self.root / "expected"
        expected.write_text("right\n")
        with self.assertRaisesRegex(RuntimeError, "baseline: stdout differs"):
            compare.compare(self.options("print('wrong')", f"--expected-stdout={expected}"))

    def test_no_warmup_still_checks_both_variants(self):
        report = compare.compare(self.options("print('answer')", "--warmups=0", "--runs=1"))
        self.assertEqual(report["warmup_samples"], [])
        self.assertEqual(len(report["samples"]), 2)

    def test_controlled_gc_environment_does_not_record_inherited_values(self):
        options = self.options("print('answer')", "--candidate-env=SCOOP_GC_FULL_ONLY=1")
        with patch.dict(os.environ, {"SCOOP_GC_STRESS_MOVE": "1", "UNRELATED_PRIVATE": "hidden"}):
            variants = compare.variants_for(options)
        self.assertNotIn("SCOOP_GC_STRESS_MOVE", variants["baseline"]["environment"])
        self.assertEqual(variants["candidate"]["environment"]["SCOOP_GC_FULL_ONLY"], "1")
        self.assertNotIn("UNRELATED_PRIVATE", variants["baseline"]["environment_overrides"])

    def test_gc_diagnostic_record_is_retained(self):
        metrics = {
            "scoop_gc": 1,
            "minor_collections": 1,
            "full_collections": 0,
            "pause_buckets": [1],
        }
        code = (
            "import os, sys; print('answer'); os.write(2, b'\\xffdiagnostic\\n'); "
            f"print({json.dumps(metrics)!r}, file=sys.stderr)"
        )
        report = compare.compare(self.options(code, "--gc-stats", "--runs=1", "--warmups=0"))
        self.assertEqual(report["samples"][0]["gc"], metrics)

    def test_gc_histogram_failure_and_missing_record(self):
        with self.assertRaisesRegex(RuntimeError, "expected one GC"):
            gc_statistics("ordinary stderr\n", "sample")
        with self.assertRaisesRegex(RuntimeError, "inconsistent pause histogram"):
            gc_statistics(
                '{"scoop_gc":1,"minor_collections":2,"full_collections":0,"pause_buckets":[1]}',
                "sample",
            )

    def test_gc_histogram_counts_incremental_pauses(self):
        metrics = {
            "scoop_gc": 1,
            "minor_collections": 1,
            "full_collections": 1,
            "pause_buckets": [2, 3],
        }
        self.assertEqual(gc_statistics(json.dumps(metrics), "sample"), metrics)

    def test_full_only_rejects_minor_collection(self):
        metrics = {
            "scoop_gc": 1,
            "minor_collections": 1,
            "full_collections": 0,
            "pause_buckets": [1],
        }
        code = f"import sys; print('answer'); print({json.dumps(metrics)!r}, file=sys.stderr)"
        with self.assertRaisesRegex(RuntimeError, "full-only control performed minor"):
            compare.compare(self.options(code, "--gc-stats", "--baseline-env=SCOOP_GC_FULL_ONLY=1"))


if __name__ == "__main__":
    unittest.main()
