# B-25 platform rtti

- Category: ../../../../audit/UMB_categories/B-25.md
- Strategy: ../../../../audit/strategies/B-25.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-25`
- Fixture index: ../_index.csv rows with `bucket=B-25`

## Scope

P7-C2 activates this fixture set for Platform and RTTI runtime lowering. The positive fixture exercises `getPlatform()`, `is`, `!is`, `as?`, and `is` patterns; the negative fixture keeps invalid runtime-cast metadata on the frontend/typecheck path.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-25 rows are retired by P7-C2 and this directory is active.
