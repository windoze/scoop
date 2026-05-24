# Claude Execution Plan

## Scope
- Current task: P10-T01R, review TypeStore wire format.
- Source of truth: TODO.md and TODO-7.md.
- Task goal: verify P10-T01 actually solved cross-process TypeId wire format, schema version coverage, and round-trip coverage for complex type shapes; fix any concrete issues found, then mark only P10-T01R complete.

## Step-by-step plan
1. Confirm repository state and latest commit handoff for any directly relevant unfinished issue.
2. Inspect P10-T01 implementation surfaces:
   - TypeStore portable serialization and TypeId validation.
   - Wire schema version type/constant and all fact/LIR product version fields.
   - Round-trip tests for TypeStore, four fact crates, and LIR program.
3. Search for gaps in persisted fact/LIR structures that still lack serde coverage or schema version fields.
4. If review finds a bug or missing coverage directly tied to P10-T01R, fix it without changing task scope; otherwise leave implementation unchanged.
5. Run required validation in the requested order:
   - cargo fmt
   - cargo clippy --all-targets -- -D warnings
   - P10-T01 targeted tests
   - cargo test --all --all-targets
   - git diff --check
6. Update TODO.md and TODO-7.md by prefixing P10-T01R with [DONE] and adding a completion record.
7. Commit all task-related changes with a descriptive P10-T01R commit message and stop.

## Progress
- Identified P10-T01R as the first incomplete task.
- Wrote this plan before running project commands.
- Latest commit is `[P10-T01] Add stable TypeStore wire format`; it directly hands off to this review and does not mention a separate unfinished blocker.
- Existing unrelated dirty files observed: `run_agent.sh` and `PLUGIN_ABI.md`; they will not be reverted or included unless the task explicitly requires them.
- Review found the wire schema/version fields are present, but test coverage for the review's generic/effect/continuation focus was too shallow.
- Added targeted bincode round-trip coverage for generic/effect/projection TypeStore shapes, populated effect step/continuation facts, and populated LIR control/continuation contracts.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- Targeted P10-T01 wire tests passed. An initial full-suite run was stopped after repeated no-output waits while `scoopc_effect_facts` appeared stalled; isolated rerun showed the tests themselves complete instantly after slow rustdoc/test startup, and the full Rust suite passed on rerun.
- Full fixture suite and `git diff --check` passed.
- Marked P10-T01R `[DONE]` in TODO.md and TODO-7.md with completion record; next step is committing the task changes.
