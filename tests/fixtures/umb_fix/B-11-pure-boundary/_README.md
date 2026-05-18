# B-11 pure boundary

- Category: ../../../../audit/UMB_categories/B-11.md
- Strategy: ../../../../audit/strategies/B-11.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-11`
- Fixture index: ../_index.csv rows with `bucket=B-11`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-11` or `ignore-until-fix:B-11` are skipped until the corresponding production fix lands.
