# B-01 builder invariant

- Category: ../../../../audit/UMB_categories/B-01.md
- Strategy: ../../../../audit/strategies/B-01.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-01`
- Fixture index: ../_index.csv rows with `bucket=B-01`

## Scope

B-01 is sentinel-only. User-facing `.scoop` fixtures cannot directly construct an invalid LLVM insertion context, so this README records where the archived retired helper-invariant IDs live after P8.

## Sentinel Coverage

- SENTINEL: `crates/scoopc/src/audit/spec_coverage.rs::umb_fix_builder_invariant_readme_records_archive`
- SENTINEL-STATUS: archived-by-P8-T02
- SENTINEL-COVERS: ARCHIVED: docs/archive/audits/unsupported-main-body/UMB_retired.csv
- SENTINEL-NOTES: P7-B1 retired these helper invariant IDs; P8-T02 archived the retired ledger and no `_index.csv` fixture row is created because the coverage anchor is a Rust fixture-set audit.

## Runner Status

The Python fixture runner (`python3 tools/run_fixtures.py`) recognizes `tests/fixtures/umb_fix/**`. B-01 remains README-only after P7-B1 because invalid builder state is an internal compiler invariant, not a user-visible fixture surface.
