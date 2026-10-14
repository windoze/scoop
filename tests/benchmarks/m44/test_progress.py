"""Reject incomplete resume observations instead of turning them into timings."""

import copy
import runpy
import unittest
from pathlib import Path

validate = runpy.run_path(str(Path(__file__).with_name("measure-progress.py")))["validate"]


class ProgressValidationTest(unittest.TestCase):
    def setUp(self):
        self.record = {
            "scoop_progress": 1,
            "mode": 0,
            "threads": 2,
            "iterations": 2048,
            "probe_every": 1024,
            "workers": [{"checksum": 2048 * 2049 // 2, "probes": 3}] * 2,
            "collections": [
                {
                    "stopping_ns": 110,
                    "collecting_ns": 120,
                    "running_ns": 160,
                    "progress": [[100, 1024, 170, 2048], [0, 0, 0, 0]],
                }
            ],
        }

    def test_actual_progress_includes_post_publication_delay(self):
        result = validate(self.record, 0, 2, 2048)
        self.assertEqual(result["request_to_running_ns"], [50])
        self.assertEqual(result["progress_gap_ns"], [70])
        self.assertEqual(result["running_to_progress_ns"], [10])

    def test_back_to_back_collections_can_share_one_later_progress_probe(self):
        second = copy.deepcopy(self.record["collections"][0])
        second.update(stopping_ns=162, collecting_ns=164, running_ns=168)
        self.record["collections"].append(second)
        result = validate(self.record, 0, 2, 2048)
        self.assertEqual(result["progress_gap_ns"], [70, 70])
        self.assertEqual(result["running_to_progress_ns"], [10, 2])

    def test_missing_first_progress_is_not_a_zero_resume_latency(self):
        self.record["collections"][0]["progress"][0][2:] = [0, 0]
        with self.assertRaisesRegex(RuntimeError, "did not make observed progress"):
            validate(self.record, 0, 2, 2048)

    def test_pre_running_observation_does_not_prove_resume(self):
        self.record["collections"][0]["progress"][0][2] = 155
        with self.assertRaisesRegex(RuntimeError, "did not make observed progress"):
            validate(self.record, 0, 2, 2048)

    def test_timestamp_without_iteration_progress_is_rejected(self):
        self.record["collections"][0]["progress"][0][3] = 1024
        with self.assertRaisesRegex(RuntimeError, "did not make observed progress"):
            validate(self.record, 0, 2, 2048)

    def test_allocation_needs_both_real_bytes_and_retry_observation(self):
        self.record.update(mode=1, allocated_bytes=0, nursery_retries=1)
        with self.assertRaisesRegex(RuntimeError, "did not allocate"):
            validate(self.record, 1, 2, 2048)
        self.record.update(allocated_bytes=2 * 2048 * 24, nursery_retries=0)
        with self.assertRaisesRegex(RuntimeError, "nursery retry hook"):
            validate(self.record, 1, 2, 2048)


if __name__ == "__main__":
    unittest.main()
