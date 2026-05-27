# B-33 extern global funptr

- Category: ../../../../audit/UMB_categories/B-33.md
- Strategy: ../../../../audit/strategies/B-33.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-33`
- Fixture index: ../_index.csv rows with `bucket=B-33`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. All B-33 fixtures are active after P8 and must remain runnable.
