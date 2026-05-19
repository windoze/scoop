# B-13 composite transport

- Category: ../../../../audit/UMB_categories/B-13.md
- Strategy: ../../../../audit/strategies/B-13.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-13`
- Fixture index: ../_index.csv rows with `bucket=B-13`

## Scope

P7-C3 retired the B-13 production fallbacks and keeps this directory as active coverage for array/composite transport metadata, task transport tuple paths, and range/progression fixtures.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-13 fixtures are active; retired UMB IDs are covered by `audit/UMB_retired.csv`, so fixture `COVERS` headers use `NONE`.
