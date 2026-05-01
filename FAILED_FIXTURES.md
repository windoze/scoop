# Failed Fixtures

## Round 1

Command shape: `target/debug/scoop test --fixtures <fixture>`

Summary:
- total: 388
- failed: 3
- timed out: 0
- per-fixture timeout: 60s

Failed fixtures:
- `tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop`
- `tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
- `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`

## Round 2

Command shape: `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 target/debug/scoop test --fixtures <fixture>`

Summary:
- total: 389
- failed: 1
- timed out: 0
- per-fixture timeout: 60s

Failed fixtures:
- `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
