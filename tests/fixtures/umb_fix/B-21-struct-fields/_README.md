# B-21 struct fields

- Category: ../../../../audit/UMB_categories/B-21.md
- Strategy: ../../../../audit/strategies/B-21.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-21`
- Fixture index: ../_index.csv rows with `bucket=B-21`

## Scope

P7-A2 retires the B-21 FrontendReject rows and activates struct-field frontend fixtures for unknown fields, missing required fields, field type mismatch, and valid struct/with-update paths. The remaining B-21 InternalBugSentinel rows stay covered until the later aggregate/member schema verifier task retires them.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. P7-A2 fixtures are active; the parse-only disambiguation fixture remains ignored until its placeholder names are replaced. Retired FrontendReject IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` fields.
