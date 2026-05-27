# Execution Plan

Note: I cannot record private chain-of-thought, so this file contains a concise operational plan and progress log.

1. Read TODO.md to identify the first task whose heading is not prefixed with [DONE].
2. Check the latest commit only for unfinished work directly relevant to that selected task.
3. Inspect the task requirements, dependencies, validation instructions, and relevant project files.
4. Implement the selected task completely, or add the minimum prerequisite task in TODO.md if a concrete blocker prevents correct implementation.
5. Run formatting, linting, relevant tests, then full required test and fixture validation unless only documentation changed since the last successful run.
6. Update TODO.md by prefixing the completed task title with [DONE] and filling its completion record; update PLAN.md only if phase-level planning changed.
7. Commit all task-related changes with a descriptive message and the required co-author trailer.
8. Stop after completing exactly this one task.

Progress:
- Initial plan recorded.
- Selected first incomplete task: P2-T06 (parse inline generic bounds and ref/value bound keywords).
- Latest commit is P2-T05R; no directly relevant unfinished issue was indicated by its subject.
- Read P2-T06 details: implement inline generic bounds and bound-only ref/value parser/type-lowering surface; keep AnyRef/AnyValue semantics for P3-T06.
- Implementation approach: add AST GenericBound for type/ref/value bounds; store inline bounds on TypeParam; update resolver/typecheck consumers to read inline + where constraints; reject ref/value in ordinary type positions.
- First compile check exposed missing ParseError span plumbing; fixed and rerunning format/check.
- Code now compile-checks after removing an unused legacy bound-lowering helper; adding targeted fixtures for inline bounds and ref/value invalid positions.
- Targeted new fixtures pass; starting full validation sequence: fmt, clippy, Rust tests, spec fixture check, full fixture suite.
- cargo fmt, clippy with -D warnings, and cargo test --all --all-targets passed.
- spec_fixtures.py check and full tools/run_fixtures.py passed.
- Full validation passed; updating TODO.md and TODO-2.md completion records for P2-T06.
- TODO.md and TODO-2.md now mark P2-T06 as [DONE] with completion details.
- Final diff review and whitespace check passed; only comments changed after full validation.
