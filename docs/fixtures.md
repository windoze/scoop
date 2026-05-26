# Fixture directives

This page records the fixture directive contract currently parsed by
`crates/scoopc/src/fixtures/expectations.rs`. These directives are read from a
fixture source file's leading line-comment header and drive the legacy
`scoop test` runner while the Python runner is being introduced.

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
