# Failed Fixtures

## Round 1

Command shape: `target/debug/scoop test --fixtures <fixture>`

Summary:
- total: 388
- failed: 2
- timed out: 0
- per-fixture timeout: 60s

Failed fixtures:
- `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`
- `tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`

## Round 2

Command shape: `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 target/debug/scoop test --fixtures <fixture>`

Summary:
- total: 389
- failed: 0
- timed out: 0
- per-fixture timeout: 60s

Failed fixtures:
- none

## Round 3

Command shape: `tools/run_fixture_scan.sh --no-build --out-dir target/fixture-scan/round3-30s`

Per-fixture command: `target/debug/scoop test --fixtures <fixture>`

Summary:
- total: 1262
- failed: 7
- timed out: 0
- per-fixture timeout: 30s

Failed fixtures:
- `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
- `tests/fixtures/run-pass/task_step_concurrent_running_trap.scoop`
- `tests/fixtures/run-pass/task_step_cross_thread_sequential_handoff_basic.scoop`
- `tests/fixtures/run-pass/task_step_manual_basic.scoop`
- `tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`
- `tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop`
- `tests/fixtures/runtime_gc/task_step_manual_gc_aggregate_transport_basic.scoop`
