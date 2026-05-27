# B-16 control flow context

- Category: ../../../../audit/UMB_categories/B-16.md
- Strategy: ../../../../audit/strategies/B-16.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-16`
- Fixture index: ../_index.csv rows with `bucket=B-16`

## Scope

P7-A1 retires the B-16 `UnsupportedMainBody` rows and activates this directory. The negative fixtures assert typecheck/front-end diagnostics for invalid `break`, `continue`, and `return` contexts; the positive fixture keeps valid loop/return lowering covered.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-16 fixtures are active after P7-A1; retired IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` fields.
