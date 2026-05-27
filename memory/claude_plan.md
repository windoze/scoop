# Execution Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work that is directly relevant to that selected task.
3. Read the selected task requirements, dependencies, validation requirements, and completion record.
4. Inspect the smallest relevant parts of the codebase needed for the selected task.
5. Implement the task as written, without narrowing scope or using workarounds for missing behavior.
6. Run formatting and validation in the requested order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, then relevant/full tests and fixtures as required.
7. If a blocking prerequisite is discovered, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
8. If the task is completed, mark its heading `[DONE]`, update its completion record with changes and validation, commit all relevant changes, and stop.

Progress:

- Initial execution plan written before reading task details.
- Read `TODO.md`; first incomplete task is `P2-T02`, covering `tools/run_fixture_scan.sh`, `tools/run_run_pass_gc_scan.sh`, and `tools/gc_microbench.sh` internal call-chain migration after completed prerequisite `P2-T02A`.
- Checked latest commit `d5280db1 [P2-T02A] Fix class ctor GC roots`; it confirms the direct prerequisite was completed and does not mention a separate unfinished issue.
- Inspected the three target shell scripts. `run_fixture_scan.sh` and `run_run_pass_gc_scan.sh` already invoke `tools/run_fixtures.py`; `gc_microbench.sh` has no fixture-runner invocation. No script code change is currently needed unless validation exposes a failure.
- Validation found a blocking `tools/run_run_pass_gc_scan.sh` timeout in `delegated_property_lazy_thread_safety_synchronized_once.scoop` under `SCOOP_GC_VERIFY_ROOTS=1 --gc-move --gc-stress`.
- Reproduction showed STW diagnostics waiting for a non-initiator thread left in `Running` while it was waiting inside thread unregister/register during an active STW. Plan update: fix `scoop_gc_thread_register` and `scoop_gc_thread_unregister` in both baseline and Immix STW implementations so already-registered non-initiator threads park before waiting for STW completion.
- Implemented the STW register/unregister fix in `runtime/c/scoop_gc.c` and `runtime/c/scoop_gc_backend_immix.c`.
- Validation completed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, full `python3 tools/run_fixtures.py`, 20x targeted GC-stress reproduction, `tools/run_fixture_scan.sh` parse scan, full `tools/run_run_pass_gc_scan.sh`, both `tools/gc_microbench.sh` smoke scenarios, and old-entry grep for `tools/*.sh` all passed.
- Updated `TODO.md` to mark `P2-T02` as `[DONE]` with completion record.
