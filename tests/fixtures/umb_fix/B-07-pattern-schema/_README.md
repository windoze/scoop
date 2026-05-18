# B-07 pattern schema

- Category: ../../../../audit/UMB_categories/B-07.md
- Strategy: ../../../../audit/strategies/B-07.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-07`
- Fixture index: ../_index.csv rows with `bucket=B-07`

## Scope

U5-T03 assigns direct B-07 inventory coverage to the positive and negative pattern schema fixtures in this directory.

## Direct Coverage

- Positive: `pos_pattern_destructuring.scoop`
- Negative: `neg_pattern_schema_gate.scoop`
- Covers: all 34 B-07 inventory ids (`UMB-1072` through `UMB-1109`, with current inventory gaps preserved).
- Upstream gate: `typecheck pattern gate: pattern subject and clause schema are valid`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-07` or `ignore-until-fix:B-07` are skipped until the corresponding production fix lands.
