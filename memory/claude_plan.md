# Execution Plan

This file records the execution plan and progress for the current invocation without private reasoning. I will use `TODO.md` as the source of truth, complete exactly the first incomplete task, commit the result, and stop.

Selected task: `P1-T01` — update pure spec decisions for `Nothing`, cone/package wording, and value type `with`.

1. Read the relevant `P1-T01` task entry in `TODO-1.md`, `SPEC_FIX.md` A1/A2/D1, and the corresponding `PLAN.md` P1 requirements.
2. Update `SCOOP_FULL_SPEC.md` only in the requested non-behavior-changing spec prose:
   - add `Nothing` to the type hierarchy as an uninhabited bottom type outside the reference/value split;
   - rewrite the §13 overview to distinguish cone as the distribution/build unit from `package` as a source namespace;
   - clarify that value types remain immutable, structs do not gain `var` fields, and `with` remains the copy-update mechanism.
3. Manually synchronize the maintained split spec files under `docs/spec/language_spec-part*.md` for the same three decisions, without changing P2/P3 surface syntax examples or fixture-affecting code blocks.
4. Run `python3 tools/spec_fixtures.py check` and manually review the new prose locations.
5. Mark `P1-T01` `[DONE]` in `TODO.md` and `TODO-1.md`, fill in its completion record, and record that split spec synchronization was manual.
6. Commit the documentation and progress-record changes with a descriptive `P1-T01` commit message and the required co-author trailer, then stop.

Progress:
- Identified `P1-T01` as the first incomplete task from `TODO.md`; latest commit is `[P0-T02R] Review overload baseline`, which does not introduce unfinished work that blocks this spec-only task.
- Reviewed the task details and relevant A1/A2/D1 plan/spec context.
- Updated `SCOOP_FULL_SPEC.md` and manually synchronized `docs/spec/language_spec-part1.md` / `language_spec-part2.md` for `Nothing`, cone/package wording, and value-type `with` immutability decisions without changing fixture-bearing code blocks.
- Validation completed: `python3 tools/spec_fixtures.py check` passed with `spec fixtures: ok (1)`, and `git diff --check` reported no whitespace errors.
- Marked `P1-T01` `[DONE]` in `TODO.md` and `TODO-1.md` with the completion record.
- Next step is to review the final diff and commit the task changes.
