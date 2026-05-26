# Execution Plan

I cannot record private chain-of-thought, but I will keep this file updated with the actionable plan, rationale summary, progress, and any changes in direction.

## Current Objective
Complete exactly the first incomplete task in `TODO.md`: P1-T03, `tools/fixtures_matrix.py {check,stdlib}` as a Python standard-library replacement for `scoop_tools fixtures-matrix`.

## Step-by-Step Plan
1. Inspect the P1-T03 task details in `TODO.md`, the relevant plan/design docs, and the existing Rust `fixtures_matrix` implementation.
2. Determine the exact command-line behavior, output, exit codes, scanned files, and validation expectations for `check` and `stdlib`.
3. Implement `tools/fixtures_matrix.py` using only the Python standard library, matching the old implementation without introducing fixture concepts into compiler crates.
4. Add or update focused tests/documentation guards only where directly relevant to this task.
5. Run formatting first, then clippy, then targeted parity checks against the old Rust command, then required full test and fixture validation if code changed.
6. Mark P1-T03 `[DONE]` in `TODO.md` and append a completion record with validation commands/results.
7. Update this plan file with the completed key steps and validation results.
8. Commit all resulting changes with a descriptive `[P1-T03]` commit message and required co-author trailer.
9. Stop after this one task.

## Progress
- Initial plan recorded.
- Identified P1-T03 as the first incomplete task.
- Inspected `TODO.md`, `PLAN.md`, `TEST_INFRA_CLEANUP.md`, and the old Rust `fixtures_matrix` implementation.
- Implemented `tools/fixtures_matrix.py` with Python standard-library code for `check` and `stdlib`.
- Verified exact current-output and exit-code parity against `cargo run -q -p scoop_tools -- fixtures-matrix check` and `stdlib`.
- Verified synthetic parity for repeated `//` directive prefixes, unmapped doctest fixtures, missing fixture files, duplicate chapter ids, unknown expectations, and stdlib domain matching.
- Completed validation: `python3 -m py_compile tools/fixtures_matrix.py`; `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all --all-targets`; `python3 tools/run_fixtures.py tests/fixtures` (1532 checks); `cargo run -p scoop -- test` (1532 checks).
- Marked P1-T03 `[DONE]` in `TODO.md` and appended its completion record.
