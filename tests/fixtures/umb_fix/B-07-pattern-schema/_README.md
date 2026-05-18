# B-07 pattern schema

- Category: ../../../../audit/UMB_categories/B-07.md
- Strategy: ../../../../audit/strategies/B-07.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-07`
- Fixture index: ../_index.csv rows with `bucket=B-07`

## Scope

P7-B2.3 activates the positive and negative pattern schema fixtures in this directory.

## Direct Coverage

- Positive: `pos_pattern_destructuring.scoop`
- Negative: `neg_pattern_schema_gate.scoop`
- Covers: no active B-07 IDs remain; retired rows live in `audit/UMB_retired.csv`.
- Upstream gate: `typecheck pattern gate: pattern subject and clause schema are valid`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-07 fixtures are active after P7-B2.3.
