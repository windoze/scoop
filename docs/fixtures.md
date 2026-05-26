# Fixture runner contract

This page records the fixture discovery and directive contract currently
implemented by `crates/scoopc/src/fixtures/mod.rs` and
`crates/scoopc/src/fixtures/expectations.rs`. The legacy `scoop test` runner
uses these rules while the Python runner is being introduced.

## Discovery roots and target planning

`scoop test` defaults to `tests/fixtures`, and `--fixtures <path>` may point at
the whole tree, a phase subtree, a multi-file case directory, a cone case
directory, or one `.scoop` file. The runner canonicalizes that path before
planning targets.

`plan_targets(root)` first recognizes roots that already are indivisible
targets:

| Root shape | Planned target |
| --- | --- |
| `<anything>.scoop` | The single file. |
| `tests/fixtures/resolve_multi/<case>/` | One resolve multi-file case. |
| `tests/fixtures/resolve_cone/<case>/` | One resolve multi-cone case. |
| `tests/fixtures/typecheck_multi/<case>/` | One typecheck multi-file case. |
| `tests/fixtures/typecheck_cone/<case>/` | One typecheck multi-cone case. |
| `tests/fixtures/run_pass_cone/<case>/` with both `Cone.toml` and `src/main.scoop` | One run-pass cone package case. |

Otherwise `plan_targets(root)` collects targets in this stable order:

1. Recursively collect all ordinary `.scoop` files below `root`, sort them, and
   add them as file targets.
2. Append sorted direct child directories from `resolve_multi/`.
3. Append sorted direct child directories from `resolve_cone/`.
4. Append sorted direct child directories from `typecheck_multi/`.
5. Append sorted direct child directories from `typecheck_cone/`.
6. Append sorted direct child directories from `run_pass_cone/`.

The recursive file scan skips:

- the dedicated case roots listed above, so a multi-file or cone case is not
  split into separate file targets;
- companion sysroot overlay directories whose names end in `.sysroot`;
- cone build output directories named `build` when they contain `debug/` or
  `release/`.

Empty target sets are accepted only for `umb_fix` subtrees and the retired
`typecheck_cone_archive` marker directory. Other empty roots fail with a "no
`.scoop` files found" diagnostic.

`tests/fixtures/cone/` is not a phase directory or case collection in the
legacy runner. It currently contains manifest-only sample data and no `.scoop`
files, so it contributes no target during full-tree planning. Passing that
directory as the fixture root would use the normal empty-root rule and fail.

Python-portable pseudocode for target planning:

```text
def plan_targets(root):
    if (
        is_file(root)
        or is_resolve_multi_case_root(root)
        or is_resolve_cone_case_root(root)
        or is_typecheck_multi_case_root(root)
        or is_typecheck_cone_case_root(root)
        or is_run_pass_cone_case_root(root)
    ):
        return [target(path=root, display=basename_or_relative(root, root))]

    resolve_multi = sorted_case_dirs(root / "resolve_multi")
    resolve_cone = sorted_case_dirs(root / "resolve_cone")
    typecheck_multi = sorted_case_dirs(root / "typecheck_multi")
    typecheck_cone = sorted_case_dirs(root / "typecheck_cone")
    run_pass_cone = sorted_case_dirs(run_pass_cone_root(root))

    skip_dirs = [
        existing(root / "resolve_multi"),
        existing(root / "resolve_cone"),
        existing(root / "typecheck_multi"),
        existing(root / "typecheck_cone"),
        existing(run_pass_cone_root(root)),
    ]

    files = sorted(recursive_scoop_files(root, skip_dirs))
    targets = [target(file, relative(root, file)) for file in files]
    targets += [target(case, relative(root, case)) for case in resolve_multi]
    targets += [target(case, relative(root, case)) for case in resolve_cone]
    targets += [target(case, relative(root, case)) for case in typecheck_multi]
    targets += [target(case, relative(root, case)) for case in typecheck_cone]
    targets += [target(case, relative(root, case)) for case in run_pass_cone]

    if not targets and (is_under_umb_fix(root) or basename(root) == "typecheck_cone_archive"):
        return []
    if not targets:
        error("no .scoop files found")
    return targets
```

The case-root predicates are intentionally shallow. The multi-file and
multi-cone predicates only check that `root` is a directory whose parent is the
matching collection directory. `is_run_pass_cone_case_root(root)` additionally
requires `root/Cone.toml` and `root/src/main.scoop`.

## Phase routing

Ordinary file targets are routed by `phase_name(root, relative_path)`:

- If the relative path has at least two components, the first component is the
  phase directory.
- If a single file is run directly from inside a known phase directory, the
  runner walks `root` ancestors and uses the nearest known phase directory.
- A root-level file with no phase directory falls back to parse.

| Directory | Phase |
| --- | --- |
| `parse/`, `spec_doctest/`, or no phase directory | `Parse` |
| `build/` | `Build` |
| `resolve/` | `Resolve` |
| `typecheck/`, `unsafe_nogc/` | `Typecheck` |
| `infer/` | `Infer` |
| `codegen/`, `run-pass/`, `runtime_gc/` | `RunPass` |
| `hir/` | `Hir` |
| `mir/`, `mir_lowered/` | `Mir` |
| `mir_materialized/` | `MirMaterialized` |
| `effect_facts/` | `EffectFacts` |
| `effect_lowered/` | `EffectLowered` |
| `scoopir/` | `ScoopIr` |
| `umb_fix/` | `UmbFix` |
| any other first component | `Unimplemented(<name>)` |

The special directories `resolve_multi/`, `resolve_cone/`,
`typecheck_multi/`, `typecheck_cone/`, and `run_pass_cone/` do not route
through this table when planned from the normal fixture root. They are planned
as case directory targets and dispatched by `run_all` before per-file phase
routing.

## Multi-file and cone case directories

Case directories are treated as one fixture target, but their check count is
the number of `.scoop` files whose expectations are evaluated inside the case.

| Directory shape | Discovery and execution contract |
| --- | --- |
| `resolve_multi/<case>/` | Direct child directories of `resolve_multi/` are cases. A case must contain at least two `.scoop` files after recursive scanning. The runner parses all case files plus sysroot files into one resolve index, then runs binding checks for each case file and applies that file's header expectations. |
| `typecheck_multi/<case>/` | Direct child directories of `typecheck_multi/` are cases. A case must contain at least two `.scoop` files after recursive scanning. The runner builds one resolve index and one type environment for all case files plus sysroot, then typechecks each file and applies that file's header expectations. |
| `resolve_cone/<case>/<cone>/` | Direct child directories of `resolve_cone/` are cases. Each case must contain at least two cone subdirectories, excluding `.sysroot` overlays. Every cone subdirectory must contain at least one `.scoop` file. Cone ids are assigned by sorted cone directory order starting at 1; sysroot uses cone id 0. Binding checks run per file with each file's own expectations. |
| `typecheck_cone/<case>/<cone>/` | Same discovery constraints and cone id assignment as `resolve_cone/`. The runner builds one cone-aware resolve index and one type environment across all cone files plus sysroot, then typechecks each file with its own expectations and finally computes layout metadata for the whole case. |
| `run_pass_cone/<case>/` | Direct child directories of `run_pass_cone/` are scheduled as cases. A valid case root must contain `Cone.toml` and `src/main.scoop`; `is_run_pass_cone_case_root` is exactly this predicate. Expectations, `ARGS:`, stdio golden directives, `SYSROOT-DEPS:`, and `RUN-MODE:` are read from `src/main.scoop`. `EXPECT: pass` runs `scoop run` in the case directory, while `EXPECT: fail` runs `scoop build` and matches the resulting diagnostic. |

`run_pass_cone_root(root)` has one subset convenience rule: if `root` itself is
named `run_pass_cone`, it is the case collection root; otherwise the runner
looks for `root/run_pass_cone`.

`typecheck_cone_archive/` is a retired archive suite. It is not an active phase
name, and the only supported behavior is that its marker directory may produce
zero planned targets.

## Companion sysroot overlays

A target may have a companion sysroot overlay directory:

| Target | Overlay directory |
| --- | --- |
| `path/to/name.scoop` | `path/to/name.sysroot/` |
| `path/to/case_dir/` | `path/to/case_dir.sysroot/` |

If the overlay directory exists, the runner adds it to the session options for
that target. Overlay directories are skipped during target discovery and cone
subdirectory enumeration. `SYSROOT-DEPS:` directives are applied in addition to
the companion overlay, and are read per source file for ordinary file targets
or from `run_pass_cone/<case>/src/main.scoop` for cone run-pass cases.

## `umb_fix` sub-routing

Files under `umb_fix/` are discovered as ordinary file targets but use dynamic
sub-routing after `IGNORE-UNTIL-FIX` handling:

- build-like `ARGS:` or `BUILD-LLVM-*` directives run the build fixture path;
- run directives, explicit stdout/stderr golden directives, or sibling
  `.stdout` / `.stderr` files run the run-pass fixture path;
- all other files run the typecheck fixture path.

## Header parsing rules

- Directives must appear in `//` comments at the start of the file. The parser
  inspects at most the first 32 physical lines and stops at the first
  non-comment line.
- Leading whitespace before `//` is ignored. Text after `//` is trimmed before
  directive matching.
- Directive names are case-sensitive, except `IGNORE-UNTIL-FIX:` also accepts
  the historical lowercase spelling `ignore-until-fix:`.
- `EXPECT: pass` is the default when no `EXPECT:` directive is present.
- Paths named by directives are interpreted relative to the fixture file's
  parent directory.

## Core expectations

| Directive | Parameters | Semantics |
| --- | --- | --- |
| `// EXPECT: pass` | none | The selected phase must succeed. This is the default. |
| `// EXPECT: ok` | none | Alias for `EXPECT: pass`. |
| `// EXPECT: fail` | none | The selected phase must fail. If the failure diagnostics match any supplied diagnostic directives, the fixture is counted as passing. |
| `// EXPECT-ERROR: <substring>` | A diagnostic substring. | For `EXPECT: fail`, the diagnostic text must contain this substring. Subprocess-wrapped diagnostics also get a normalized text comparison. |
| `// EXPECT-ERROR-CODE: <code>` | A stable diagnostic code such as `scoop::parse::expected`. | For `EXPECT: fail`, the emitted diagnostic code must exactly match this value. |
| `// EXPECT-ERROR-AT: <line>:<col>` | 1-based source line and column. | For `EXPECT: fail`, the diagnostic primary label must resolve to this source location. |

If an `EXPECT: fail` fixture omits all `EXPECT-ERROR*` directives, any failure
from the selected phase is accepted.

## Shared runner inputs

| Directive | Parameters | Semantics |
| --- | --- | --- |
| `// ARGS: <args...>` | Whitespace-separated argument tokens. May appear more than once. | Appends tokens to the fixture argument list. Consumers are phase-specific: build fixtures inspect emit and optimization flags; run-pass fixtures pass tokens to `scoop run`; run-pass cone fixtures pass tokens to cone-mode `scoop run`; typecheck fixtures inspect `--target-platform`. |
| `// ENV: KEY=VALUE [KEY=VALUE...]` | One or more whitespace-separated key-value tokens. May appear more than once. | Adds environment variables for run-pass command execution. Tokens without `=` or with an empty key are ignored; empty values are allowed. |
| `// SYSROOT-DEPS: <cone> ...` | Cone names separated by whitespace or commas. May appear more than once. | Extends the fixture session with additional sysroot cone dependencies before phase execution. |
| `// IGNORE-UNTIL-FIX:<bucket>` | A non-empty bug or bucket id, for example `B-15`. | Skips the fixture before phase dispatch and prints a skip line. |
| `// ignore-until-fix:<bucket>` | Same as above. | Historical lowercase alias for `IGNORE-UNTIL-FIX:`. |

Known `ARGS:` consumers:

| Phase | Accepted argument shapes | Effect |
| --- | --- | --- |
| `build` | `--emit-llvm`, `--emit-obj`, `--emit-asm` | Selects the artifact kind to emit. Without one of these flags, build fixtures are no-ops. |
| `build` | `-O2`, `-Os`, `-Oz`, `-O <level>`, `--opt-level <level>`, `--opt-level=<level>` | Sets a fixture-local optimization level when no global `scoop test` optimization level overrides it. |
| `run-pass`, `codegen`, `runtime_gc` | Any tokens | Passed after the fixture path to `scoop run`. |
| `run_pass_cone` cases | Any tokens; `--release` has special handling | Passed to cone-mode `scoop run`; `--release` also changes the expected output profile to `release`. |
| `typecheck`, `unsafe_nogc` | `--target-platform <id>`, `--target-platform=<id>` | Overrides the type environment's target platform for platform-gating diagnostics. |

## Parse fixtures

| Directive | Parameters | Semantics |
| --- | --- | --- |
| `// EXPECT-AST: <file>` | Golden file path. | Parse fixtures compare the pretty debug AST dump (`{ast:#?}\n`) against the named golden file after newline normalization. If omitted, parse fixtures only require parsing to succeed or fail according to `EXPECT:`. |

## Build fixtures

| Directive | Parameters | Semantics |
| --- | --- | --- |
| `// BUILD-LLVM-CONTAINS: <substring>` | Non-empty LLVM IR substring. May appear more than once. | Requires the emitted LLVM IR to contain each listed substring. |
| `// BUILD-LLVM-REGEX: <regex>` | Non-empty Rust regex pattern. May appear more than once. | Requires the emitted LLVM IR to match each listed regex. Invalid regex syntax fails the fixture. |
| `// BUILD-LLVM-NOT-CONTAINS: <substring>` | Non-empty LLVM IR substring. May appear more than once. | Requires the emitted LLVM IR not to contain any listed substring. |

LLVM IR assertions require `ARGS: --emit-llvm`. They are not valid with
`--emit-obj` or `--emit-asm`.

## Run-pass fixtures

Run-pass applies to `tests/fixtures/codegen`, `tests/fixtures/run-pass`, and
`tests/fixtures/runtime_gc`; cone-mode run-pass cases consume the same stdout,
stderr, stdin, exit, timeout, and environment directives from
`src/main.scoop`.

| Directive | Parameters | Semantics |
| --- | --- | --- |
| `// RUN-STDOUT: <file>` | Golden file path. | Captured stdout, after `\r\n` to `\n` normalization, must exactly match the named file. If omitted, stdout is not exact-matched. |
| `// RUN-STDERR: <file>` | Golden file path. | Captured stderr, after newline normalization, must exactly match the named file. If omitted, stderr is not exact-matched. |
| `// RUN-STDIN: <file>` | Input file path. | Reads the named file as bytes, writes it to child stdin, then closes stdin so the program observes EOF. |
| `// RUN-MODE: run` | none | Default mode. Runs `scoop run <fixture>`. |
| `// RUN-MODE: dump-stackmaps` | none | Builds the fixture to a temporary executable, then runs `scoop dump-stackmaps --verify-roots <exe>` instead of running the program itself. |
| `// RUN-STDOUT-CONTAINS: <substring>` | A stdout substring. | Captured stdout, after newline normalization, must contain this substring. |
| `// RUN-STDERR-CONTAINS: <substring>` | A stderr substring. | Captured stderr, after newline normalization, must contain this substring. |
| `// RUN-STACKMAPS-RECORDS-GT: <n>` | Unsigned integer. | Parses a `records: <n>` line from stdout and requires the actual records count to be greater than the supplied value. Intended for `RUN-MODE: dump-stackmaps`. |
| `// EXPECT-EXIT: <code>` | Signed process exit code. | Overrides the default success-only exit policy. The process must exit with exactly this code, so non-zero exits can be expected. |
| `// TIMEOUT: <ms>` | Unsigned millisecond timeout. | Applies a timeout to the external run command; timing out fails the fixture with a timeout diagnostic. |

Without `EXPECT-EXIT`, run-pass commands must exit successfully. With
`EXPECT-EXIT`, a signal termination still fails because no numeric exit code is
available.
