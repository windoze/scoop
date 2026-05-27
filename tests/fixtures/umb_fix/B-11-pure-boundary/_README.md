# B-11 pure boundary

- Category: ../../../../audit/UMB_categories/B-11.md
- Strategy: ../../../../audit/strategies/B-11.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-11`
- Fixture index: ../_index.csv rows with `bucket=B-11`

## Scope

P7-B2.8 retires B-11 by verifying typed HIR local val, assignment place, and while condition statement boundaries before LLVM lowering. Fixtures in this directory remain active smoke/negative coverage after the retired IDs move to `audit/UMB_retired.csv`.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-11 fixtures are active after P7-B2.8; cross-bucket B-10 coverage remains listed only for active B-10 IDs.
