# Execution Plan

I will not record private chain-of-thought, but I will keep this file updated with a concise, auditable plan and progress log.

Selected task: P2-T02, the first incomplete task in `TODO.md`.

Plan:
1. Read `TODO-2.md` task details for P2-T02 and any validation requirements.
2. Locate parser, AST, diagnostics, docs/spec, and fixture uses of handler `with` syntax.
3. Change handler arm syntax from `with` to `on`, ensuring old handler `with` is rejected by a clear parser diagnostic while value-update `with` remains accepted.
4. Migrate positive docs, Rust snippets, and fixtures to `on`; add a negative parse fixture for old handler `with`.
5. Regenerate changed parse/HIR/MIR/effect golden files where source span shifts affect snapshots.
6. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
7. Update `TODO.md` and `TODO-2.md` completion records, then commit task changes.

Progress:
- Initial plan recorded.
- Identified P2-T02 as the first incomplete task from `TODO.md`.
- Confirmed latest commit is P2-T01R and does not add an unfinished relevant blocker.
- Found parser currently expected `Keyword::With` in `parse_handle_expr`; value-update `with` was handled separately in postfix parsing.
- Implemented `Keyword::On`, updated lexer/parser acceptance, and added `scoop::parse::handler_with_keyword_removed` for old handler `with`.
- Migrated active specs, Rust snippets, and positive handler fixtures to `on`; added `tests/fixtures/parse/handle_with_keyword_removed.scoop`.
- Regenerated affected parser/HIR/MIR/effect snapshots.
- Completed validation: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, targeted fixtures, `cargo test --all --all-targets`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py` all passed.
- Updated `TODO.md` and `TODO-2.md` to mark P2-T02 done with completion record.
- Committed the P2-T02 implementation changes in Git.

Status: P2-T02 is complete; stop after final response.
