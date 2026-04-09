# Current Task: T0114 — `Bool.toString()` + `print`/`println` Bool overload

## Status: IN PROGRESS

## Task Description
- Add `Bool.toString()` method: resolver whitelist + codegen inline `if tag == 1 then "true" else "false"`
- Add `print(value: Bool)` / `println(value: Bool)` sysroot overloads via `toString()` + existing `print(String)` routing

## Execution Plan

1. [ ] Resolver — add `Bool.toString()` to method whitelist
2. [ ] Typecheck — add `Bool.toString()` signature (0 args → String)
3. [ ] Codegen — implement `Bool.toString()` inline (select "true"/"false" global string)
4. [ ] Sysroot — add `print(Bool)` / `println(Bool)` overloads
5. [ ] Codegen — implement `print(Bool)` / `println(Bool)` routing
6. [ ] Add run-pass fixture: `bool_to_string_print_basic.scoop`
7. [ ] Test & validate (cargo test --all + cargo run -p scoop -- test)
8. [ ] Update TODO.md + PLAN.md, commit
