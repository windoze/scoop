# B-05 mir cfg

- Category: ../../../../audit/UMB_categories/B-05.md
- Strategy: ../../../../audit/strategies/B-05.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-05`
- Fixture index: ../_index.csv rows with `bucket=B-05`

## Scope

U5-T01 creates this directory skeleton only. U5-T02 and U5-T03 must add `.scoop` fixtures and append matching `_index.csv` rows.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-05` or `ignore-until-fix:B-05` are skipped until the corresponding production fix lands.
