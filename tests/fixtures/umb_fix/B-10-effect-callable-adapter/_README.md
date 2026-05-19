# B-10 effect callable adapter

- Category: ../../../../audit/UMB_categories/B-10.md
- Strategy: ../../../../audit/strategies/B-10.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-10`
- Fixture index: ../_index.csv rows with `bucket=B-10`

## Scope

P7-C5 activates this fixture directory for the retired B-10 effect callable adapter / ABI routing implementation. Retired UMB IDs are covered by `audit/UMB_retired.csv`; active fixture headers use `COVERS: NONE`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. All B-10 fixtures are active after P8 and must remain runnable.
