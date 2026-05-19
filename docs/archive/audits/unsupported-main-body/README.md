# UnsupportedMainBody Audit Archive

This directory archives the P7/P8 retirement audit data for `LlvmEmitError::UnsupportedMainBody`.

- `UMB_inventory_initial.csv`: immutable initial baseline with 1,284 stable IDs.
- `UMB_retired.csv`: final retired ledger covering all 1,284 IDs.
- `UMB_inventory_final_empty.csv`: final active inventory header with 0 active entries.
- `UMB_inventory_schema.md`: schema used by the retired audit data.

The production enum variant and `umb-audit` inventory generator were removed during P8. Long-term fixture coverage is retained by `crates/scoopc/src/audit/spec_coverage.rs` and `tests/fixtures/umb_fix/**`.
