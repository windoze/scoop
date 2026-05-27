# B-15 when pattern

- Category: ../../../../audit/UMB_categories/B-15.md
- Strategy: ../../../../audit/strategies/B-15.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-15`
- Fixture index: ../_index.csv rows with `bucket=B-15`

## Scope

- P7-A3 retired the B-15 `UnsupportedMainBody` rows and keeps `when` user-facing failures in typecheck.
- Active negative fixtures cover non-exhaustiveness, non-Bool guards, unknown variants, payload arity, and result type mismatch.
- Positive fixtures keep enum/or-pattern/guard and tuple+nested-enum `when` codegen on the normal path.

## Runner Status

Run with `python3 tools/run_fixtures.py tests/fixtures/umb_fix/B-15-when-pattern/`. All B-15 fixtures are active; retired B-15 IDs are covered by `audit/UMB_retired.csv` rather than fixture `COVERS` headers.
