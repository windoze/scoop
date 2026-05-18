# B-01 builder invariant

- Category: ../../../../audit/UMB_categories/B-01.md
- Strategy: ../../../../audit/strategies/B-01.md
- Spec matrix rows: ../../../../audit/spec_coverage_matrix.md entries containing `B-01`
- Fixture index: ../_index.csv rows with `bucket=B-01`

## Scope

B-01 is sentinel-only. User-facing `.scoop` fixtures cannot directly construct an invalid LLVM insertion context, so U5-T03 records the bucket coverage here and U6 owns the Rust sentinel test.

## Sentinel Coverage

- SENTINEL: `crates/scoopc/src/audit/sentinel_tests.rs::umb_fix_helper_invariant_sentinel_tests_present`
- SENTINEL-STATUS: planned-for-U6
- SENTINEL-COVERS: UMB-0020, UMB-0064, UMB-0093, UMB-0118, UMB-0119, UMB-0123, UMB-0124, UMB-0163, UMB-0164, UMB-0296, UMB-0297, UMB-0344, UMB-0345, UMB-0352, UMB-0353, UMB-0433, UMB-0434, UMB-0444, UMB-0445, UMB-0456, UMB-0457, UMB-0468, UMB-0469, UMB-0479, UMB-0480, UMB-0541, UMB-0542, UMB-0607, UMB-0609, UMB-0762, UMB-0763, UMB-0764, UMB-0765, UMB-0766, UMB-0767, UMB-0838, UMB-0839, UMB-0840, UMB-0848, UMB-0849, UMB-0852, UMB-0853, UMB-0854, UMB-0857, UMB-0858, UMB-0878, UMB-0880, UMB-0888, UMB-0889, UMB-0925, UMB-0926, UMB-0943, UMB-0944, UMB-0985, UMB-1025, UMB-1065, UMB-1066, UMB-1067, UMB-1068, UMB-1069, UMB-1101, UMB-1102, UMB-1155, UMB-1156, UMB-1165, UMB-1166, UMB-1174, UMB-1175, UMB-1266, UMB-1267, UMB-1269
- SENTINEL-NOTES: U5-T03 coverage record for helper invariant IDs; no `_index.csv` fixture row is created because the coverage anchor is a Rust sentinel test.

## Runner Status

`scoop test` recognizes `tests/fixtures/umb_fix/**`. Fixtures marked `IGNORE-UNTIL-FIX:B-01` or `ignore-until-fix:B-01` are skipped until the corresponding production fix lands.
