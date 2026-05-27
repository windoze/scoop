# B-08 member store

- Category: ../../../../audit/UMB_categories/B-08.md
- Strategy: ../../../../audit/strategies/B-08.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-08`
- Fixture index: ../_index.csv rows with `bucket=B-08`

## Scope

P7-A2 retired the B-08 FrontendReject rows and activated assignment/member-store frontend fixtures. P7-B2.8 retires the remaining B-08 InternalBugSentinel rows through MIR/materialized MIR member-store verifier coverage.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-08 fixtures are active; retired IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` fields.
