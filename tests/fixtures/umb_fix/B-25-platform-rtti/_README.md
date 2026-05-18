# B-25 platform rtti

- Category: ../../../../audit/UMB_categories/B-25.md
- Strategy: ../../../../audit/strategies/B-25.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-25`
- Fixture index: ../_index.csv rows with `bucket=B-25`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-25` or `ignore-until-fix:B-25` are skipped until the corresponding production fix lands.
