# B-18 literals strings

- Category: ../../../../audit/UMB_categories/B-18.md
- Strategy: ../../../../audit/strategies/B-18.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-18`
- Fixture index: ../_index.csv rows with `bucket=B-18`

## Scope

P7-B2.5 activates this directory as literal and string regression coverage after retiring the B-18 `UnsupportedMainBody` rows.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-18 fixtures are active; retired B-18 IDs are covered by `audit/UMB_retired.csv`.
