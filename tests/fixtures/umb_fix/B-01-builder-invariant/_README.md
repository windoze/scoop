# B-01 builder invariant

- Category: ../../../../audit/UMB_categories/B-01.md
- Strategy: ../../../../audit/strategies/B-01.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-01`
- Fixture index: ../_index.csv rows with `bucket=B-01`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-01` or `ignore-until-fix:B-01` are skipped until the corresponding production fix lands.
