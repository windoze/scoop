# Repository Guidelines

## Project Structure & Module Organization

- `crates/scoop/`: driver CLI (`scoop`) used to run fixtures and (optionally) build/run programs.
- `crates/scoopc/`: compiler library; LLVM codegen is gated behind the `llvm` feature.
- `crates/scoop_runtime/`: runtime build glue (Rust) + GC/runtime integration tests.
- `runtime/c/`: C runtime implementation (GC, effects, threads, platform backends).
- `stdlib/` / `sysroot/`: standard library sources and sysroot layout used by the compiler.
- `tests/fixtures/**`: fixture-based regression suite, grouped by phase (e.g. `parse/`, `typecheck/`, `run-pass/`, `spec_doctest/`).
- `tools/scoop_tools/` + `tools/*.sh`: repo utilities (spec fixture sync/check, GC microbench).

## Build, Test, and Development Commands

```bash
# Build the whole workspace (LLVM not required)
cargo build

# Run Rust unit + integration tests (CI uses this)
cargo test --all

# Check spec doctest fixtures generated from SCOOP_FULL_SPEC.md
cargo run -p scoop_tools -- spec-fixtures check

# Run fixture suite (routes by tests/fixtures/* phase)
cargo run -p scoop -- test
```

LLVM backend (default): requires `clang` and `llvm-config` in `PATH` (use `--no-default-features` to disable).

```bash
cargo build -p scoopc
cargo run -p scoop -- build path/to/file.scoop -o /tmp/a.out
```

## Coding Style & Naming Conventions

- Rust formatting: use `cargo fmt` (rustfmt defaults; workspace edition is 2024).
- Prefer idiomatic Rust: `snake_case` for modules/functions, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Fixture files use the `.scoop` extension; keep names descriptive and phase-appropriate.

## Testing Guidelines

- Rust tests live alongside code (`#[test]`) and as integration tests in `crates/scoop_runtime/tests/*.rs`.
- Fixture regressions live under `tests/fixtures/**`; add new cases to the smallest relevant phase directory.
- When editing `SCOOP_FULL_SPEC.md` code blocks tagged as fixtures, run:
  `cargo run -p scoop_tools -- spec-fixtures sync` and then `... check`.

## Commit & Pull Request Guidelines

- Commit subjects commonly start with a task tag, e.g. `[T1505a] Describe change` (see `PLAN.md` / `TODO.md`).
- PRs should include: problem statement, linked task/issue, test commands run, and any LLVM/clang requirements for reviewers.
