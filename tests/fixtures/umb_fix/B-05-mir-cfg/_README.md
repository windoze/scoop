# B-05 mir cfg

- Category: ../../../../audit/UMB_categories/B-05.md
- Strategy: ../../../../audit/strategies/B-05.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-05`
- Fixture index: ../_index.csv rows with `bucket=B-05`

## Scope

P7-B2.2 keeps MIR CFG/start block/terminator target contracts verified before LLVM codegen. The fixtures in this directory are active regression anchors for the retired B-05 bucket.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-05 fixtures are active; retired IDs are tracked in `audit/UMB_retired.csv` rather than fixture `COVERS` headers.
