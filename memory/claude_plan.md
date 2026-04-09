# Current Task: T0140 — Multi-file Literal Support

## Status: COMPLETED

## Problem
`LiteralKind::Int` and `LiteralKind::String` store no data — they rely on `self.source.slice(span)` at codegen time. But `MainCodegen.source` only holds the entry file's `SourceFile`, so non-entry file spans would slice wrong text. The `MultiFileNonEntrySourceBackedLiteral` guard prevents this but blocks stdlib files from using int/string literals.

## Approach: Store parsed values in `LiteralKind` at HIR lowering time
This avoids the need for a multi-file SourceMap in codegen. The HIR lowering context already has access to each file's `SourceFile`.

## Summary of Changes
- `LiteralKind::Int` → `LiteralKind::Int(u128)`, `LiteralKind::String` → `LiteralKind::String(Vec<u8>)`
- `WhenPat::IntLit` now carries parsed `value: u128`
- `parse_int_literal_decimal` extracted to `syntax/int_literal.rs` (shared module)
- HIR lowering (expr.rs, patterns.rs) parses literal values at lowering time
- Codegen reads stored values instead of slicing source text
- Removed `MultiFileNonEntrySourceBackedLiteral` guard and related utility functions
- Lifted member_funs restriction: now collects from all files
- `MainCodegen.source` retained (still needed for interpolated strings)
- All 12 HIR golden tests regenerated
- New cone fixture: `multi_file_literal_basic` (Int, String, arithmetic in non-entry file)
- All tests pass: 139 unit + 802 fixtures
