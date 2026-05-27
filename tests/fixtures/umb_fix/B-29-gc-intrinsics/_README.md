# B-29 gc intrinsics

- Category: ../../../../audit/UMB_categories/B-29.md
- Strategy: ../../../../audit/strategies/B-29.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-29`
- Fixture index: ../_index.csv rows with `bucket=B-29`

## Scope

P7-B3.4 retires the B-29 GC intrinsic contract rows. These fixtures remain active smoke coverage for the positive pin/handle path and the negative `GC.pin` ref-type gate; retired ID coverage lives in `audit/UMB_retired.csv`.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-29 fixtures are active after P7-B3.4.
