# B-28 thread intrinsics

- Category: ../../../../audit/UMB_categories/B-28.md
- Strategy: ../../../../audit/strategies/B-28.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-28`
- Fixture index: ../_index.csv rows with `bucket=B-28`

## Scope

P7-B3.2 keeps active positive and negative coverage for the thread intrinsic contract after retiring the B-28 inventory rows.

## Direct Coverage

- Positive: `pos_thread_intrinsics.scoop`
- Negative: `neg_thread_intrinsic_gate.scoop`
- Retired rows: all 20 B-28 IDs are tracked in `audit/UMB_retired.csv`.
- Upstream gate: `intrinsic resolver gate: thread primitive calls match sysroot runtime contract`.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-28 fixtures are active after P7-B3.2.
