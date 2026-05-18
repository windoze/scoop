# B-08 member store

- Category: ../../../../audit/UMB_categories/B-08.md
- Strategy: ../../../../audit/strategies/B-08.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-08`
- Fixture index: ../_index.csv rows with `bucket=B-08`

## Scope

P7-A2 retires the B-08 FrontendReject rows and activates the assignment/member-store frontend fixtures. The remaining B-08 InternalBugSentinel rows stay covered by `COVERS` until the later MIR/member-store verifier task retires them.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. B-08 fixtures are active after P7-A2; retired FrontendReject IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` fields.
