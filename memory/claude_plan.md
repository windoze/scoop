# Claude Execution Plan

## Scope

- Execute exactly the first incomplete task from `TODO.md`, then stop.
- Use `TODO.md` as the source of truth for task ordering, requirements, dependencies, validation, and completion records.
- Update this file as key steps complete or if the plan changes.

## Execution Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Inspect recent Git context only as needed to detect an unfinished issue directly relevant to that task.
3. Read the task body, dependencies, validation requirements, and any relevant project files.
4. Implement the task as written, unless a concrete prerequisite/blocker makes that impossible.
5. If blocked by an unscheduled prerequisite or failing test/fixture, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping, and stop.
6. Run formatting, linting, relevant tests, then full validation as required by the task and repository policy.
7. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
8. Commit all intended changes with a clear task-specific commit message.
9. Stop without starting the next task.

## Current Status

- Plan file initialized before code execution or shell commands.
- `TODO.md` read. First incomplete task: `T2-08：lowering 产出 LIR 指令（state 拥有指令）`.
- Confirmed prerequisite `T2-08A` is marked `[DONE]`.
- Git context checked: latest commit is `[T2-08A] Add LIR executable body containers`; working tree only contains this plan file.
- Relevant code paths inspected: `effect_lowered/ir.rs`, `instruction.rs`, `segment.rs`, `builder.rs`, `materialize/*`, LIR facts builder, and LLVM effect body codegen.
- Implemented an initial LIR producer path: MIR statements lift into `LirStatement`, `LateLoweredState` carries `LirStateBody`, and executable callables get `LirExecutableBody` payloads.
- Blocker found: strict `T2-08` deletion of `LateLoweredStateSlice` / `LateLoweredSourceBody` is not safe until facts builder, dump/verify, and LLVM effect body/layout consumers stop using source-slice identities.
- `TODO.md` updated with new prerequisite `T2-08B：迁移 source-slice 消费侧到 LIR body anchors`; `T2-08` remains incomplete and now depends on `T2-08B`.
- Validation run for the partial/prerequisite bookkeeping state: `cargo fmt`; `cargo check --all-targets` passed after formatting.

## T2-08 Implementation Steps

1. Add a MIR-to-LIR lift module that converts MIR statements/rvalues into the T2-07 LIR instruction types and rejects `Todo` / `UnresolvedName` instead of representing them.
2. Change `LateLoweredState` so it owns `LirStateBody` and exposes statements/terminator from that body; remove `LateLoweredStateSlice` as final IR state.
3. Thread lifted state bodies through segmentation/materialization/frame/opt paths, preserving state graph semantics.
4. Replace source-slice statement classification and boundary consumption identities with LIR body anchors.
5. Attach a `LirExecutableBody` to each executable callable and remove `LateLoweredSourceBody`/MIR body exposure from `LateLoweredProgram`.
6. Update facts builder/dump/verify/codegen call sites to consume LIR-owned body/anchors.
7. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, full tests, builds, dependency gate, spec fixtures, and full fixtures.

## Plan Change

- The original T2-08 plan cannot be completed safely in this invocation without first migrating all source-slice consumers.
- Stop after committing the prerequisite insertion and partial producer groundwork; the next invocation should pick up `T2-08B` as the first incomplete task.
