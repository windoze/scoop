# B-36 spec uncovered

- Category: ../../../../audit/UMB_categories/B-36.md
- Strategy: ../../../../audit/strategies/B-36.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-36`
- Fixture index: ../_index.csv rows with `bucket=B-36`

## Scope

P7-A4 activates this directory for spec-uncovered surfaces. Async/await and generator/yield remain intentionally undefined and must fail in frontend resolution before LLVM codegen; annotation and spec-meta files are smoke coverage without active UMB ids.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-36 fixtures are active after P7-A4; retired B-36 inventory ids are covered by `audit/UMB_retired.csv`, not active `COVERS` headers.
