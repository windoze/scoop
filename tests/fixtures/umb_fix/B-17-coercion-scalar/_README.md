# B-17 coercion scalar

- Category: ../../../../audit/UMB_categories/B-17.md
- Strategy: ../../../../audit/strategies/B-17.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-17`
- Fixture index: ../_index.csv rows with `bucket=B-17`

## Scope

P7-B2.5 activates this directory as scalar coercion/operator regression coverage after retiring the B-17 `UnsupportedMainBody` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-17 fixtures are active; retired B-17 IDs are covered by `audit/UMB_retired.csv`, while cross-bucket B-31 coverage remains pending where explicitly listed.
