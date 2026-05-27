# B-26 atomic intrinsics

- Category: ../../../../audit/UMB_categories/B-26.md
- Strategy: ../../../../audit/strategies/B-26.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-26`
- Fixture index: ../_index.csv rows with `bucket=B-26`

## Scope

P7-B3.3 retired all B-26 atomic intrinsic `UnsupportedMainBody` rows. The fixtures in this directory are active smoke coverage for the raw atomic positive path, the `AtomicValue<T>` value-bound negative gate, and raw atomic target lvalue/mutability gates.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-26 retired IDs are tracked in `audit/UMB_retired.csv`; fixture `COVERS` fields use `NONE` after retired-ledger takeover.
