# B-21 struct fields

- Category: ../../../../audit/UMB_categories/B-21.md
- Strategy: ../../../../audit/strategies/B-21.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-21`
- Fixture index: ../_index.csv rows with `bucket=B-21`

## Scope

P7-A2 retired the B-21 FrontendReject rows and activated struct-field frontend fixtures for unknown fields, missing required fields, field type mismatch, and valid struct/with-update paths. P7-B2.3 retires the remaining B-21 InternalBugSentinel rows through aggregate/member schema verifier coverage.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. P7-A2 fixtures remain active; the parse-only disambiguation fixture remains ignored until its placeholder names are replaced. All retired B-21 IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` fields.
