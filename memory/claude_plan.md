# Current task plan

Task: P4-T03R, "Review vararg overlap reject".

First incomplete task source: `TODO.md` lists P4-T03 as `[DONE]` and P4-T03R as the first task still marked `[TODO]`. The latest commit is `[P4-T03] Reject vararg overload overlaps`, which is directly relevant and is the implementation under review.

Execution plan:

1. Read the P4-T03 completion record, P4-T03R requirements, and relevant overload-resolution design sections.
2. Inspect the P4-T03 implementation in overload definition-time checks, vararg parameter validation, call-argument mapping assumptions, diagnostics, and targeted fixtures.
3. Verify the required review points: overlap rejected at definition time, non-overlap legal cases pass, diagnostics include candidate locations/signatures, and spread syntax cannot bypass the definition-time reject.
4. Make any required corrections discovered during review; if no implementation changes are needed, only update task documentation.
5. Run formatting/linting and the targeted/full validation required for the review task, reusing prior green results only if no compiled-output-affecting files change.
6. Mark P4-T03R `[DONE]` in `TODO.md` and `TODO-4.md`, fill in its completion record, and commit only the task-related changes.

Progress:

- Identified P4-T03R as the first incomplete task.
- Reviewed the P4-T03 implementation against `OVERLOAD_RESOLUTION.md` §4.3 / §8.2. The definition-time check runs before ordinary conflict/default checks, includes candidate spans and rendered signatures, and the call mapper treats surviving vararg candidates as already disambiguated.
- Adding one targeted negative fixture so the review requirement that spread syntax cannot bypass the definition-time reject is covered directly.
- Validation passed: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, targeted vararg fixtures, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and full fixtures (`fixtures: ok (1562)`).
- Marked P4-T03R as `[DONE]` in `TODO.md` and `TODO-4.md`; `PLAN.md` did not require changes.
