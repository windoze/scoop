# B-12 closure capture

- Category: ../../../../audit/UMB_categories/B-12.md
- Strategy: ../../../../audit/strategies/B-12.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-12`
- Fixture index: ../_index.csv rows with `bucket=B-12`

## Scope

P7-C4 activates this fixture set for closure/lambda/capture lowering. Retired B-12 IDs are tracked in `audit/UMB_retired.csv`; active fixtures use `COVERS: NONE`.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. All B-12 fixtures are active after P8 and must remain runnable.
