# Claude Execution Plan

Note: This file records the actionable plan and progress updates for the current invocation. It intentionally avoids private chain-of-thought while preserving the steps needed to audit progress.

## Initial Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect that task's requirements, dependencies, and validation instructions, plus any immediately relevant recent commit context.
3. Implement the selected task completely, without working around missing language/runtime behavior.
4. Run formatting, linting, relevant tests, and required fixture validation in the prescribed order.
5. Update `TODO.md` to prefix the completed task heading with `[DONE]` and fill/update its completion record.
6. Commit all changes for this invocation with a descriptive task-tagged message and stop without starting the next task.

## Progress

- Plan file initialized.

- Identified first incomplete task: P2-T01 (`perform` prefix removal and effect op call migration).
- Latest commit is `[P1-T02R] Review Inline removal`; no directly relevant unfinished issue noted in the commit subject.
- Next: inspect parser/typecheck/lowering handling and all `perform` fixtures/usages.

- Confirmed `Effect.op(args)` already reaches effect operation typechecking/lowering through ordinary qualified call dispatch.
- Implementation decision: keep `perform` lexed as a reserved keyword only to produce a targeted removal diagnostic; remove parser acceptance of prefix `perform expr`.
- Next: patch parser diagnostics, update examples/tests/spec snippets, and add a negative parse fixture for old syntax.

- Patched parser to reject `perform` with `scoop::parse::perform_keyword_removed`.
- Added a negative parse fixture for old `perform` syntax.
- Rewrote Rust test snippets and active spec examples from `perform Effect.op(...)` to `Effect.op(...)`.
- Next: run formatting, linting, targeted validation, full Rust tests, and full fixture suite.

- Targeted fixture run showed `perform_keyword_removed.scoop` unexpectedly succeeded.
- Next: inspect parser behavior and fixture runner path, then fix the root cause before continuing validation.

- Formatting, spec fixture check, clippy, CLI rebuild, and targeted perform fixtures passed after correcting the new fixture expectation.
- Next: run the full Rust test suite, then the full fixture suite.

- Full Rust test suite passed.
- Full fixture suite passed.
- Next: update TODO task status/completion record and commit the completed P2-T01 changes.

- Updated `TODO.md` and `TODO-2.md` to mark P2-T01 `[DONE]` with completion details.
- No phase-level plan change was needed, so `PLAN.md` remains unchanged.
- Next: run final diff checks and commit all invocation changes.

- Final diff check passed.
- Committing P2-T01 completion changes now.
