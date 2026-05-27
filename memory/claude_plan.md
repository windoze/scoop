# Claude Execution Plan

Note: This file records the actionable plan and progress updates for the current invocation. It intentionally avoids private chain-of-thought while preserving the steps needed to audit progress.

## Current task

First incomplete task: `P2-T01R` in `TODO-2.md`, mirrored in the root `TODO.md` index.

Goal: review the completed `P2-T01` removal of the `perform` prefix syntax and confirm that ordinary qualified effect operation calls still work end-to-end.

## Plan

1. Check the latest commit message for any unfinished issue directly relevant to `P2-T01R`.
2. Inspect the `P2-T01` implementation surfaces:
   - parser keyword/prefix handling;
   - effect-op typecheck binding;
   - HIR/MIR lowering paths;
   - fixtures and spec snippets touched by the `perform` migration.
3. Search active code, sysroot, specs, and fixtures for remaining `perform` usage.
4. Confirm old `perform` syntax appears only in diagnostics, negative fixtures, or historical/design baseline documents.
5. Confirm at least one migrated positive effect-op fixture uses direct `Effect.op(args)` form and still passes.
6. Run required validation in the requested order:
   - `cargo fmt`;
   - `cargo clippy --all-targets -- -D warnings`;
   - targeted effect-op/perform fixtures;
   - full `cargo test --all --all-targets`;
   - full `python3 tools/run_fixtures.py`.
7. If the audit finds a defect, fix it in scope or add the minimum prerequisite task before `P2-T01R` and stop.
8. If the audit passes, mark `P2-T01R` as `[DONE]` in both `TODO.md` and `TODO-2.md`, fill its completion record, and commit the task result.

## Progress

- Identified `P2-T01R` as the first incomplete task.
- Latest commit is `[P2-T01] Remove perform prefix syntax`; no unfinished issue in the subject directly changes `P2-T01R`.
- Parser audit confirms `perform` is still lexed as a reserved keyword, but expression parsing now emits `scoop::parse::perform_keyword_removed` instead of accepting `perform expr`.
- Typecheck/lowering audit confirms ordinary qualified member calls still route resolved effect operations through `infer_effect_op_call_expr_type` and `try_lower_effect_op_call_expr`.
- Fixture/spec search found old prefix syntax only in the negative parse fixture and diagnostic/help text; other active `perform` occurrences are comments, prose, or internal runtime terminology.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed.
- Initial targeted fixture command used multiple positional paths, but this runner accepts one path per invocation; rerunning the same targeted fixtures as separate commands.
- Targeted `perform`/effect-op fixtures passed individually.
- Full `cargo test --all --all-targets` passed.
- Full `python3 tools/run_fixtures.py` passed with `fixtures: ok (1535)`.
- Marked `P2-T01R` as `[DONE]` in both `TODO.md` and `TODO-2.md` and filled its completion record.
