# 本轮执行计划

## 说明

按要求先记录可审计的执行摘要、判断依据和步骤计划，不写原始内部推理草稿。

## 上一轮上下文

1. 上一轮唯一执行单元是 `P6-T02R`。
2. 该 review 任务在复审 `P6-T02` / `P6-T02a` 时发现新的真实 blocker：`materialize_resume_interface_layout(...)` 只校验了已发布 method 自身是否匹配 step shell，但没有把 published case 集与 authoritative `LateLoweredStepType` 中同一 effect family 应有的完整 case 集做最终比对。
3. 因此上一轮没有把 `P6-T02R` 标记完成，而是在 `TODO-P6.md` 中新增了前置任务 `P6-T02b`，并同步了 `TODO.md`。

## 当前轮目标

1. 读取 `TODO.md`，根据索引顺序定位对应的 `TODO-Px.md` 文件。
2. 在详细任务文件中找到第一个标题未带 `[DONE]` 的任务，并核对其约束、依赖、验证要求与完成记录。
3. 检查最近一次提交是否存在与该任务直接相关且未完成的事项；若是，则并入当前任务或作为前置依赖记录。
4. 在不偏离规范、不使用 workaround 的前提下，实现该任务所需改动。
5. 运行与该任务直接相关的验证、测试与必要的静态检查；若发现阻塞问题，先修复阻塞问题，或按要求在对应 `TODO-Px.md` / `TODO.md` 中新增最小前置任务并停止。
6. 完成后更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]`，补充完成记录；如任务索引信息发生变化，同步更新 `TODO.md`。仅当阶段计划发生变化时更新 `PLAN.md`。
7. 检查工作区未提交改动，按要求提交本次任务相关变更，然后停止，不继续下一个任务。

## 当前执行步骤

1. 定位首个未完成详细任务，并确认最新提交是否直接对应该任务。
2. 审阅 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 中 `materialize_resume_interface_layout(...)` 的现状，确认缺少的是 authoritative method completeness 校验，而不是 interface identity/order 问题。
3. 在 layout materializer 中补上按 `(return_step_schema, effect_family)` 对 authoritative case 集的完整性比对。
4. 新增一个“故意删掉 authoritative method”的定向测试；若旧测试构造依赖了不完整 shell，同时修正该测试构造，但不改变它原本要验证的 contract。
5. 运行任务要求的定向测试、fixture 验证与 `clippy -D warnings`。
6. 回写 `TODO-P6.md`、同步 `TODO.md`，然后提交并停止。

## 当前进度

- 已读取 `TODO.md` 与 `TODO-P6.md`，确认首个未完成详细任务为 `P6-T02b`。
- 已检查最新提交：`[P6-T02b] Track resume-interface method completeness blocker`，与当前任务直接相关，说明该任务就是上一轮 review 新增的 blocker 修复项。
- 已完成实现：`materialize_resume_interface_layout(...)` 现在会把 authoritative step shell 中、同一 `(return_step_schema, effect_family)` 下应发布的 case 集与 `LateLoweredResumeInterface.methods()` 实际发布的 case 集做最终比对；若缺失 method，会以结构化错误 fail fast，并指出 interface id、step schema、effect family 与缺失 case tag。
- 已保持原 contract：vtable index 仍严格按 `LateLoweredResumeInterface.methods()` 的发布顺序分配，新增逻辑只做 completeness 校验，不补造、不重排 method shell。
- 已补充测试：新增 `refactor_llvm_continuation_layout_rejects_missing_authoritative_method`；同时把 `refactor_llvm_continuation_layout_preserves_authoritative_interface_order` 的输入改为先构造完整 Ping method 集，再仅验证 interface 顺序，避免旧测试继续依赖不完整 shell。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 接下来只剩检查工作区、生成本次任务提交并停止。
