# Current task plan

Task: P4-T03, "实现 vararg 与非 vararg overlap 的定义点 reject".

First incomplete task source: `TODO.md` points to `TODO-4.md`; the index shows P4-T03 as the first task not marked `[DONE]`. The latest commit is `[P4-T02R] Review generic overload shape rules`, which is directly adjacent but does not name an unfinished issue.

Execution plan:

1. Read the full P4-T03 entry in `TODO-4.md`, plus the relevant overload design/spec sections.
2. Inspect the current definition-time overload checks and tests/fixtures around varargs and duplicate overloads.
3. Implement the vararg/non-vararg overlap reject at the declaration/typecheck definition point, reusing the existing effective-signature helpers and diagnostics style.
4. Add or update the smallest relevant fixtures/tests required by P4-T03.
5. Run formatting, clippy, Rust tests, spec fixture check, and fixture suite as required by the task and project instructions.
6. Mark P4-T03 `[DONE]` in both `TODO.md` and `TODO-4.md`, update the completion record, and commit all task changes.

Progress:

- Identified P4-T03 as the first incomplete task.
- Read the P4-T03 requirements and relevant overload implementation. Current signature metadata records defaults and effective types but not vararg-ness, so the implementation will extend `ParamInfo` and add a dedicated vararg/non-vararg overlap check before the existing duplicate/default conflict branches.
- Implemented vararg-aware overload signature metadata, added `scoop::typecheck::vararg_overlaps_non_vararg`, and added targeted rejected/legal vararg overload fixtures. Targeted fixtures pass after rebuilding the compiler binaries.
- Full validation passed (`cargo fmt`, clippy, build, targeted fixtures, full Rust tests, spec fixture check, full fixture suite). Marked P4-T03 done in `TODO.md` and `TODO-4.md`.
