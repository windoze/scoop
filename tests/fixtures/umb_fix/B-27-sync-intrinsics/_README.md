# B-27 sync intrinsics

- Category: ../../../../audit/UMB_categories/B-27.md
- Strategy: ../../../../audit/strategies/B-27.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-27`
- Fixture index: ../_index.csv rows with `bucket=B-27`

## Scope

P7-B3.2 keeps active positive and negative coverage for the sync intrinsic contract after retiring the B-27 inventory rows.

## Direct Coverage

- Positive: `pos_sync_intrinsics.scoop`
- Negative: `neg_sync_intrinsic_gate.scoop`
- Retired rows: all 58 B-27 IDs are tracked in `audit/UMB_retired.csv`.
- Upstream gate: `intrinsic resolver gate: sync primitive calls match sysroot runtime contract`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-27 fixtures are active after P7-B3.2.
