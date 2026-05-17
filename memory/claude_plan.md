# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to the next task after completion.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit only for directly relevant unfinished work tied to that task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the task as specified, avoiding workarounds or scope narrowing.
5. Run focused validation first, then broader required validation for the task.
6. Fix any task-relevant failures discovered during validation.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding a completion record.
8. Update this file whenever the plan changes or a key step completes.
9. Commit all changes for the completed task with a clear task-tagged message.
10. Stop after the commit.

## Progress

- Initial execution plan written before reading project task files or running commands.
- Identified first incomplete task: `P9-T03` in `TODO-5.md`, dependent on completed `P9-T02`.
- Latest commit is `[P9-T02] Migrate stdlib-dependent fixtures`, with no separate unfinished issue indicated by the subject.
- Impact check found frontend support-source injection, const/comptime stdlib loading, and build fingerprint stdlib hashing that must be removed together with the `stdlib/` directory.
- Removed project stdlib injection/loading/hash paths and deleted the tracked `stdlib/` source directory.
- Post-edit static checks show no remaining `stdlib/` path references in Rust sources or TOML; `frontend.rs` has no `stdlib` matches.
- Validation passed so far: `cargo build`; full fixture suite `cargo run -p scoop -- test` passed with 1345/1345 targets and 1382 checks.
- Validation completed: `cargo clippy --all-targets -- -D warnings` passed; `cargo test --all --all-targets` passed; `ls stdlib/` reports no such directory; tracked `stdlib/*` files are gone; `frontend.rs` has no `stdlib` matches.
- `TODO.md` and `TODO-5.md` updated to mark `P9-T03` as `[DONE]` with completion details.
