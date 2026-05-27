# B-23 member access

- Category: ../../../../audit/UMB_categories/B-23.md
- Strategy: ../../../../audit/strategies/B-23.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-23`
- Fixture index: ../_index.csv rows with `bucket=B-23`

## Scope

U5-T03 assigns direct B-23 inventory coverage to the positive and negative member-access fixtures in this directory.

## Direct Coverage

- Positive: `pos_member_access_dispatch.scoop`
- Negative: `neg_member_access_contract.scoop`
- Covers: all 24 B-23 inventory ids (`UMB-0021`, `UMB-0026`, `UMB-0049`, `UMB-0060`, `UMB-0872`-`UMB-1224` as listed in `_index.csv`).
- Upstream gate: `typecheck member-access gate: receiver and target member are resolved`.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. All B-23 fixtures are active after P8 and must remain runnable.
