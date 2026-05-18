# B-24 reflection comptime

- Category: ../../../../audit/UMB_categories/B-24.md
- Strategy: ../../../../audit/strategies/B-24.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-24`
- Fixture index: ../_index.csv rows with `bucket=B-24`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-24` or `ignore-until-fix:B-24` are skipped until the corresponding production fix lands.
