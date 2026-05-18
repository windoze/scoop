# B-27 sync intrinsics

- Category: ../../../../audit/UMB_categories/B-27.md
- Strategy: ../../../../audit/strategies/B-27.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-27`
- Fixture index: ../_index.csv rows with `bucket=B-27`

## Scope

U5-T03 adds the B-27 negative gate fixture so the sync intrinsic bucket has both positive and negative coverage.

## Direct Coverage

- Positive: `pos_sync_intrinsics.scoop`
- Negative: `neg_sync_intrinsic_gate.scoop`
- Covers: all 58 B-27 inventory ids in the negative fixture; the positive fixture also covers related sync/thread/sysroot ids shared with adjacent buckets.
- Upstream gate: `intrinsic resolver gate: sync primitive calls match sysroot runtime contract`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-27` or `ignore-until-fix:B-27` are skipped until the corresponding production fix lands.
