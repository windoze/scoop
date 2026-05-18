# B-23 member access

- Category: ../../../../audit/UMB_categories/B-23.md
- Strategy: ../../../../audit/strategies/B-23.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-23`
- Fixture index: ../_index.csv rows with `bucket=B-23`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-23` or `ignore-until-fix:B-23` are skipped until the corresponding production fix lands.
