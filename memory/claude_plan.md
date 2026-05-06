# Execution Plan

## Scope

- Current authoritative task source: `TODO.md`.
- First incomplete task: `MIR-T03` (`收口 parser/frontend/HIR placeholder 入口`).
- Complete exactly `MIR-T03`, mark it `[DONE]`, commit, then stop.
- Do not broaden into unrelated historical issues unless they directly block `MIR-T03`.

## Step-By-Step Plan

1. Check the latest commit message for unfinished work directly relevant to `MIR-T03`.
2. Inspect the HIR placeholder inventory, HIR/refactor stage preflight, parser/frontend diagnostics, and existing tests for `spawn`, `join`, parser recovery `Missing`, and package-level `comptime if`.
3. Extend HIR placeholder inventory so entries use the same disposition vocabulary as MIR inventory and fail on untracked placeholder ingress.
4. Add or tighten parser/frontend diagnostics so deferred `spawn` and user-facing `join` are rejected before HIR/MIR production output.
5. Ensure parser recovery `Missing` cannot enter refactor production HIR/MIR as a successful output; upgrade it to source diagnostics in the refactor frontend path.
6. Ensure untrimmed package-level `comptime if` reports a source diagnostic instead of lowering to `Item::Todo(comptime_if_item)`.
7. Add HIR stage no-placeholder preflight so refactor successful HIR output rejects required-to-eliminate `ExprKind::Todo`, `StmtKind::Todo`, `Item::Todo`, and `ExprKind::Missing` before MIR fallback.
8. Add or update targeted unit tests and diagnostics fixtures required by `TODO.md`.
9. Run the required validation: `cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory` and `cargo test -p scoopc --no-default-features refactor_hir_preflight`, plus any newly relevant targeted fixture/diagnostic commands.
10. Fix task-scoped validation failures without weakening behavior or using workarounds.
11. Update `TODO.md` by prefixing `MIR-T03` with `[DONE]` and adding a completion record with validation results.
12. Update this file after key milestones or if the plan changes.
13. Commit all invocation changes with a `MIR-T03` scoped message.
14. Stop without beginning `MIR-T04`.

## Progress

- Plan initialized for this invocation before running terminal commands.
- `TODO.md` was read to identify the first incomplete task: `MIR-T03`.
- Latest commit checked: `fae27801 [MIR-T02] Add materialized MIR verifier`; no directly relevant unfinished issue found in the commit subject.
- Working tree contains existing `MIR-T03`-scoped edits in HIR/MIR placeholder and preflight files plus one resolve fixture; next step is to inspect and complete them without reverting unrelated work.
- Inspection found existing edits for the main `MIR-T03` surfaces: HIR placeholder inventory disposition names, parser `spawn`/`join` rejection, HIR lowering stage errors for parser recovery `Missing`, package-level `comptime if`, and a HIR completeness verifier on `TypedHirStageOutput::new`.
- Next step is targeted validation to identify any remaining compile/test issues.
- Required tests passed once: `refactor_hir_placeholder_inventory` and `refactor_hir_preflight`; both exposed an unused-variable warning in `hir/lower/expr.rs`, which has been fixed by marking the placeholder span parameter intentionally unused.
- Final validation passed after formatting: `refactor_hir_placeholder_inventory`, `refactor_hir_preflight`, `refactor_hir_no_todo`, `parser_hir_surface_gate`, six targeted diagnostics fixtures for `spawn`/`join`/parser recovery/package-level comptime-if, and `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`.
- `TODO.md` has been updated to mark `MIR-T03` as `[DONE]` with completion notes and validation results.
- Final step: inspect git status/diff, commit all `MIR-T03` changes, then stop.
