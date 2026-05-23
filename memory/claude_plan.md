# Current Invocation Plan

## Scope

- Follow `TODO.md` as the authoritative task order.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical triage before selecting the current task.
- If a concrete blocker or unscheduled failing test/fixture prevents completion, update `TODO.md` with the minimum required prerequisite/follow-up task, commit that bookkeeping, and stop.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check recent git context only as needed for that selected task, including whether the latest commit explicitly references an unfinished issue relevant to it.
3. Inspect the code, fixtures, and documentation directly relevant to the selected task.
4. Implement the task with the smallest spec-correct change; avoid workarounds or narrowed fixture shapes.
5. Run targeted validation first, then broader required validation for the task. Address any observed unscheduled failures according to the failure policy.
6. Update `TODO.md` by prefixing the task title with `[DONE]` and recording the completion details, or add a prerequisite/follow-up task if completion is blocked.
7. Update this plan file after key progress points or any plan change.
8. Inspect `git status`, `git diff`, and recent commits, then commit all intended changes with a task-specific message.
9. Stop without starting the next task.

## Execution Progress

- Initial plan file created before command execution.
- Selected first incomplete task: `P8-T01` in `TODO-6.md` (`最终 residual 搜索、文档冻结与未来 C backend 输入边界`).
- Latest commit `3e935453 [P7-T05R] Review P7 backend cleanup` directly supports this task by confirming P7 cleanup and leaving P8 as final residual/documentation freeze; no separate prerequisite was identified from the commit subject.

## Current Task Execution Plan

1. Read P8-T01 requirements and relevant P7 completion context.
2. Run targeted residual searches for old comptime/const surface, devirtualization ownership drift, stage-output nesting, upstream-wrapper handoff residuals, HIR/raw MIR/effect facts LLVM coupling, and duplicate fact-table fallback patterns.
3. Inspect dependency gate and documentation to see which final constraints are automated and where the future C backend boundary is currently described.
4. Make the smallest necessary source/documentation/dependency-gate changes so the final boundary is explicit and regressions are checked mechanically where feasible.
5. Run required validation for P8-T01: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo test --all --all-targets`, and `git diff --check`.
6. Update `TODO.md` and `TODO-6.md` with `[DONE]` status and completion records if validation passes.
7. Commit the completed task and stop.

## Residual Findings

- Active source still tokenized `comptime` as a dedicated keyword (`Keyword::Comptime`) and had a parser test explicitly expecting that keyword. This is a P0/P8 residual because old comptime surface should fail naturally as ordinary parse/resolve/typecheck behavior, not by preserving a dedicated surface token.
- LLVM still contained `codegen/main/const_eval.rs` with `const_eval_*` helpers and an unused `const_initializer_for_top_level_var` helper. The P0/P8 cleanup guidance allows backend data initializers when needed, but this helper is currently dead and keeps old const-evaluator naming alive.

## Plan Update

- Remove the active `Comptime` keyword and replace the parser/lexer tests with coverage that `comptime` is now a normal identifier.
- Delete the dead LLVM `const_eval` module and unused top-level-var constant initializer helper instead of renaming unused code.
- Extend `dependency-gate` with final P8 source-boundary rules to prevent reintroducing `Comptime` keyword tokens or LLVM `const_eval` helpers.

## Implementation Progress

- Removed `Keyword::Comptime`, the lexer keyword mapping, parser recovery/display references, and updated parser/lexer regressions so `comptime` is a normal identifier.
- Deleted the unused LLVM `main/const_eval.rs` module and unused top-level var const initializer helper; updated the failure-policy audit baseline accordingly.
- Added dependency-gate source boundary checks for the old comptime keyword surface and LLVM const-eval residuals.
- Updated active docs and LIR type-context wire owner from the stale `P8/per-cone` wording to `P10 per-cone build artifact serialization`, with effect-lowered goldens synchronized.
- Targeted checks passed: parser unit tests, lexer regression, parse fixtures, effect-lowered fixtures, and dependency gate.
- Required `cargo test --all --all-targets` exposed `fixtures::tests::run_all_recreates_session_between_independent_fixtures` failing on `mir_lowered/aggregate_transport.scoop` MIR golden drift. This must be fixed or explicitly scheduled before P8-T01 can be marked complete.
- Reproduced the drift on the standalone `mir_lowered/aggregate_transport.scoop` fixture and regenerated its MIR golden; the standalone fixture now passes, and the original unit test passed before the filtered cargo invocation exceeded the short timeout while entering unrelated integration test binaries.
- Re-running `cargo test --all --all-targets` passed the earlier MIR fixture test but exceeded the 20 minute tool timeout while entering `scoop_runtime` integration test `gc_immix_write_barrier`; this must be investigated under the single-test timeout policy.
- `gc_immix_write_barrier` passed standalone, confirming the prior timeout was total-suite timeout rather than a stuck single test. A longer full-suite run then exposed stale `scoopc` baselines: LIR tests used the wrong TypeStore owner, native extern declarations missed the gc-leaf attribute on LIR-declared symbols, an LLVM stage test expected the old callable key shape, and the failure-policy sentinel list was stale. These have been updated and the targeted tests now pass.
- Final validation passed: `cargo fmt`, `cargo run -p scoop_tools -- dependency-gate`, `cargo run -p scoop_tools -- spec-fixtures check`, `cargo test --all --all-targets` with extended timeout, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
- `TODO.md` and `TODO-6.md` have been updated to mark `P8-T01` as `[DONE]` with a completion record. `PLAN.md` was not changed because no phase-level sequencing or dependency changed.

# P8-T01R Invocation Plan

## 执行计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务，确认任务正文、依赖和验证要求。
2. 检查最近提交信息，仅在其明确提到与当前任务直接相关的未完成问题时纳入当前任务或作为前置项记录到 `TODO.md`。
3. 读取当前任务涉及的代码、测试、文档和已有夹具，明确最小正确实现范围，避免绕过规范或削弱测试。
4. 实现当前任务；若遇到阻塞当前任务的缺失语言特性、规范不匹配或未排期失败测试，优先修复，或在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行当前任务要求的验证命令和相关回归测试；若发现未排期失败，修复或排入当前任务之前。
6. 完成后更新 `TODO.md`：在当前任务标题前加 `[DONE]`，填写完成记录和验证结果；仅当阶段级计划改变时才更新 `PLAN.md`。
7. 运行格式化或必要质量检查，检查工作区差异，提交本次任务所有相关更改。
8. 提交后停止，不继续下一个任务。

进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 选择第一个未完成任务。
- 已读取 `TODO.md` 与 `TODO-6.md`，第一个未完成任务为 `P8-T01R：Review final residual 搜索与文档冻结`。
- 最新提交为 `706f34ba [P8-T01] Freeze final backend residual boundary`，与当前 review 直接相关，将作为复审对象。
- 本任务执行步骤：检查工作区状态；复审 P8-T01 的 residual 搜索覆盖、dependency gate 规则和文档冻结；运行 P8-T01 验证矩阵及 clippy；如发现真实回退则修复；最后同步 `TODO.md` / `TODO-6.md` 完成记录并提交。
- 工作区状态：除本计划文件外，已有未跟踪 `PLUGIN_ABI.md`；它未被当前任务引用，暂不改动或提交。
- 初步 review：dependency gate 已覆盖旧 `comptime` keyword/lexer/parser surface、LLVM `const_eval` module、top-level const initializer helper、LLVM stage/emit/reachability/codegen context、legacy HIR function declaration/emission/identity、call lowering/ABI/source lookup、dispatch helper、ordinary callee analysis、class ctor body等边界。
- 初步 residual 搜索：旧 comptime/const_eval/top-level const helper在 `crates/scoopc/src` 无生产命中；LLVM 普通去虚化无命中；LLVM wrapper/pass-view residual 仅见 layout 测试；pipeline 中的 P3/P4 output 交叉引用是 effect lowering 显式输入和“输出不保存 wrapper”的注释，符合 P8-T01 记录。
- 验证已通过：`cargo fmt`；`cargo run -p scoop_tools -- dependency-gate`；`cargo run -p scoop_tools -- spec-fixtures check`；`cargo test --all --all-targets`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- 已将 `TODO.md` 与 `TODO-6.md` 中 `P8-T01R` 标记为 `[DONE]` 并填写完成记录；`PLAN.md` 未修改，因为阶段计划没有变化。
