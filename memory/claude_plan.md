# Claude Plan

## Note

按要求先写计划文件，再开始读取和执行仓库任务。此文件记录可审计的执行计划、决策依据摘要、关键进展与必要调整；不记录私有逐词推理。

## Initial Execution Plan

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务，确认它是本次唯一执行目标。
2. 检查最近提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务前置条件，则按要求在 `TODO.md` 中显式记录并停止继续后续实现。
3. 阅读当前任务涉及的说明、依赖、验收与 completion record，并只调查完成该任务所需的最小代码范围。
4. 如任务可直接实现，进行最小且正确的代码修改；如遇到会阻塞该任务的真实缺口或 spec mismatch，则先在 `TODO.md` 中添加最小前置任务并调整顺序。
5. 运行任务要求的验证，以及必要的回归测试；如果修改影响范围明确扩大，则补充相应测试。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 更新 `TODO.md`：仅当任务真正完成时，将该任务标题前缀改为 `[DONE]` 并补全完成记录；若被阻塞，则保持未完成并记录新的前置任务。
8. 仅当阶段级计划发生变化时更新 `PLAN.md`；否则不改。
9. 按要求创建一次 git 提交，提交本次任务相关的全部未提交改动，然后停止，不继续下一个任务。

## Current Task

- 第一个未完成任务：`CG-T07S0a4`。
- 任务目标：修复 `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop` 在默认 full-suite 下触发的 `assignment place contract references an unallocated local`，解除 `CG-T07S0a` 的新 blocker。

## Root Cause Summary

- 已复现：`cargo run -p scoop -- build tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop -o /tmp/kotlin_ranges_progressions_basic` 会在 `crates/scoopc/src/mir/lower.rs:2453` panic。
- 单独 `dump-mir stdlib/prelude.scoop` 可成功 lower `IntProgression.forEach`，说明原始单文件 typed-HIR -> MIR 主线正常。
- build 走的是 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources`，会在 explicit MIR instance lowering 中重新 lower 同一源码函数体。
- `AssignPlaceContract` 侧表按 `source_path + stmt span` 查找，但 contract 内部保存的 `SymbolId` 来自某个 `HirLowering` 上下文；`SymbolInterner` 目前是每个 `HirLowering` 私有分配，因此同一 `decl_span` 在不同 lowering 上下文里的 `SymbolId` 可漂移。
- MIR lowering 当前只按 `SymbolId -> LocalId` 取目标 local，遇到 monomorphic instance body 时，contract 命中的是同源码 span 的 authoritative 记录，但其中 `SymbolId` 可能不属于当前函数体，于是触发 `unallocated local` panic。

## Planned Fix

1. 在 MIR `FnLowering` 中为 assign-place local contract 增加 `decl_span` 重绑定逻辑。
2. 消费 `AssignPlaceKind::Local` contract 时，先按 `SymbolId` 查；若当前函数体没有该 symbol，则按 contract 自带的 `name + decl_span` 在当前 MIR body 的 source local 中选取最窄匹配声明 span，重新绑定当前 body 的 `LocalId`。
3. 保持 typed assign-place contract 仍是 authoritative source；只修正 explicit MIR instance re-lowering 时 local identity 的再绑定方式，不放宽 validator、也不把问题推迟到 backend。
4. 增加 build-like 回归测试，覆盖 `kotlin_ranges_progressions_basic.scoop` 通过 production single-file codegen frontend 准备阶段。
5. 运行任务要求的 build / fixture / full-suite / clippy 验证，之后更新 `TODO.md` 并提交。

## Progress Log

- 已创建计划文件。
- 已读取 `TODO.md`，定位当前任务为 `CG-T07S0a4`。
- 已确认最近提交已把该 blocker 显式补录进 `TODO.md`，无需再新增 prerequisite。
- 已完成复现与根因定位，下一步开始实施 `decl_span -> LocalId` 重新绑定修复。
- 已在 `crates/scoopc/src/mir/lower.rs` 落地修复：assign-place local contract 命中 stale `SymbolId` 时，改按 contract 自带的 `name + decl_span` 在当前 MIR body 的 source local 中重新绑定目标 local。
- 已新增回归测试：`llvm::tests::production_codegen_progression_fixture_prepares_generic_for_each_assign_contracts`。
- 已完成定向验证：
  - `cargo test -p scoopc production_codegen_progression_fixture_prepares_generic_for_each_assign_contracts`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop -o /tmp/kotlin_ranges_progressions_basic`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 默认 `cargo run -p scoop -- test` 已越过 `kotlin_ranges_progressions_basic.scoop`，新的下一处 blocker 为 `tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`，build 诊断：`materialized MIR 'scoop.core.push' contains unresolved generic parameter in array transport element type ...: T`。
- 已更新 `TODO.md`：
  - 将 `CG-T07S0a4` 标记为 `[DONE]` 并补全完成记录。
  - 新增 prerequisite `CG-T07S0a5`，供下次调用处理。
