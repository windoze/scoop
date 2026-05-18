# B-30 named unsafe funptr

- Category: ../../../../audit/UMB_categories/B-30.md
- Strategy: ../../../../audit/strategies/B-30.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-30`
- Fixture index: ../_index.csv rows with `bucket=B-30`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-30` or `ignore-until-fix:B-30` are skipped until the corresponding production fix lands.
