# Current task plan

Task: P4-T04, "实现 override / overload 边界与虚方法 generic 禁止".

First incomplete task source: `TODO.md` marks P4-T03R as `[DONE]` and P4-T04 as the first `[TODO]` entry. The latest commit is `[P4-T03R] Review vararg overload overlap reject`, which is a direct predecessor but does not identify a separate unfinished blocker.

Execution plan:

1. Read the P4-T04 requirements plus the referenced overload-resolution sections for override/overload boundaries and virtual generic methods.
2. Inspect the current implementation in inheritance, interface conformance, override effect checks, overload helpers, modifier metadata, and existing fixtures.
3. Implement definition-time rules:
   - parent non-open same signature is rejected even if the child spells `override`;
   - parent open same signature requires `override`;
   - `override` with no matching parent signature is rejected;
   - same-name different-signature child methods remain legal overloads;
   - method-level type parameters are rejected on open, abstract, override, and interface methods while class/interface-level type parameters remain legal;
   - override target lookup ignores effect rows, leaving effect compatibility to the existing override-effects pass.
4. Add targeted typecheck fixtures covering the four override boundary cases, virtual generic rejection, class/interface-level type parameter allowance, and effect-row lookup behavior if not already covered.
5. Run formatting, linting, targeted fixtures, full Rust tests, spec fixture check if relevant, and full fixture suite.
6. Mark P4-T04 `[DONE]` in both `TODO.md` and `TODO-4.md`, fill its completion record, update this file with validation progress, and commit the task changes.

Progress:

- Identified P4-T04 as the first incomplete task.
- Implemented signature-based override/interface matching using lowered parameter types with owner type-argument substitution, added virtual generic method rejection, and added targeted P4-T04 fixtures.
- Ran `cargo fmt` and fixed clippy context-size findings by consolidating pass state into local context structs.
- Added the virtual-generic diagnostic to the earlier annotation pass so missing-body checks cannot mask it for `abstract fun <T>`.
- Migrated the sysroot `Map<K, V>.getValue` surface from a virtual method-level generic to the interface-level `V`, and fixed intrinsic interface override matching to keep method names part of interface member matching.
- Validation passed: `cargo clippy --all-targets -- -D warnings`; `cargo build -p scoop -p scoopc`; targeted override/interface fixtures for P4-T04; rerun of `commands::run::tests::run_builds_and_executes_minimal_main`.
- Full validation passed after synchronizing generated HIR/effect-lowered snapshots: `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Marked P4-T04 as `[DONE]` in `TODO.md` and `TODO-4.md`; `PLAN.md` did not require changes.
