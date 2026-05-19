# B-12 closure capture

- Category: ../../../../audit/UMB_categories/B-12.md
- Strategy: ../../../../audit/strategies/B-12.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-12`
- Fixture index: ../_index.csv rows with `bucket=B-12`

## Scope

P7-C4 activates this fixture set for closure/lambda/capture lowering. Retired B-12 IDs are tracked in `audit/UMB_retired.csv`; active fixtures use `COVERS: NONE`.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-12 fixtures are active after P7-C4 and should not be marked `IGNORE-UNTIL-FIX:B-12`.
