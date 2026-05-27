# B-06 literal schema

- Category: ../../../../audit/UMB_categories/B-06.md
- Strategy: ../../../../audit/strategies/B-06.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-06`
- Fixture index: ../_index.csv rows with `bucket=B-06`

## Scope

P7-B2.3 activates the B-06 aggregate/tuple/schema fixture set. Retired B-06 IDs are tracked in `audit/UMB_retired.csv`; active `COVERS` fields only keep cross-bucket IDs that still remain in inventory.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-06 fixtures are active after P7-B2.3; some rows still mention B-13/B-23 because those cross-bucket IDs remain active for later tasks.
