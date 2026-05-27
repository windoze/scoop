# Execution Plan

## Selected task

First incomplete task: `P2-T03` in `TODO-2.md` / `TODO.md`: implement tuple field access syntax `.0` / `.1` and remove positive `._0` usage.

Latest commit completed `P2-T02R` and does not mention an unfinished issue that preempts `P2-T03`.

## Step-by-step plan

1. Inspect tuple field parsing, tokenization, typechecking, lowering, codegen helper parsing, and tuple-related fixtures/docs.
2. Update lexer/parser behavior so `x.1.2` is tokenized/parsed as chained numeric member access while ordinary float literals like `1.2` still work.
3. Represent numeric member/path segments through the existing AST member-name path where possible, avoiding a broad AST rewrite unless required.
4. Update tuple index parsing in typecheck/lowering/codegen helpers from `_N` to numeric `N`, while rejecting old tuple spelling as positive surface.
5. Migrate positive fixtures, Rust embedded Scoop snippets, spec text, and snapshots from `._0` / `._1` to `.0` / `.1`; add or keep a stable negative fixture for old `._N` spelling if diagnostics are stable.
6. Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, targeted tuple/float fixtures, `cargo test --all --all-targets`, and the full fixture suite.
7. Mark `P2-T03` as `[DONE]` in `TODO-2.md` and `TODO.md`, fill the completion record, commit all task-related changes, and stop.

## Progress

- Implemented lexer state for numeric member tokens after `.` / `?.` while preserving ordinary float literals.
- Updated parser member access and `with` field paths to accept numeric tuple segments, including `with { 1.0: ... }` path splitting.
- Switched tuple index parsing and generated HIR tuple member names to numeric `N`, with typecheck diagnostics for old `_N` / `._N` tuple syntax.
- Migrated active positive tuple fixtures and spec snippets to `.0` / `.1`; added old-syntax negative fixtures.
- Completed formatting, clippy, targeted tuple validation, full Rust tests, and full fixture validation.
- Marked `P2-T03` done in `TODO.md` and `TODO-2.md`; next step is committing this task only.
