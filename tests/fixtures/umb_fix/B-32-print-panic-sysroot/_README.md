# B-32 print panic sysroot

- Category: ../../../../audit/UMB_categories/B-32.md
- Strategy: ../../../../audit/strategies/B-32.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-32`
- Fixture index: ../_index.csv rows with `bucket=B-32`

## Scope

P7-B3.1 retired the B-32 sysroot print/panic bridge rows. This directory now keeps active positive and negative smoke coverage; retired IDs live in `audit/UMB_retired.csv`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-32 fixtures are active after P7-B3.1.
