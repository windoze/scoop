# 执行计划记录

## 说明

根据当前会话约束，这里记录可审计的执行计划、关键判断、已完成步骤与后续调整，不写出逐字内部思维内容。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交内容，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前总体规划与任务依赖关系。
4. 评估第一个未完成任务是否足够小且可直接完成。
5. 若任务过大，则拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，本轮仅执行拆分后的第一个子任务。
6. 为当前任务建立实现上下文：阅读相关源码、测试、规范或文档。
7. 实现任务所需修改，避免引入规避性方案；若发现既有缺陷或规格不匹配，优先修复，或把其登记为阻塞前置任务并停止。
8. 运行与改动相关的测试；如有必要，补充测试。
9. 运行格式化、静态检查与所需验证，确保无警告。
10. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
11. 提交 Git commit，提交信息描述本轮完成的任务。
12. 停止，不继续处理下一个任务。

## 当前状态

- 已完成：创建计划记录文件并写入初始执行步骤。
- 已完成：检查最新提交 `2c380459 [T5000c1] Extract shared ProgramFacts side table`，提交说明中未提到需要优先修复的额外既有问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，定位本轮第一个未完成任务为 `T5000c1R Review：确认 ProgramFacts 已成为 backend-agnostic 的共享 side table`。
- 已完成：初步审查 `crates/scoopc/src/program_facts.rs`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`，确认：
  - `ProgramFacts` 已抽到独立模块，结构本身只依赖 HIR lowering 与类型系统，不依赖 LLVM builder / module / GC ABI。
  - 正式构造入口已统一到 `ProgramFacts::from_lowered(&hir::LoweredHir)`；生产代码路径由 `llvm/emit.rs` 统一构造，effect 相关测试 helper 也改为复用同一 builder。
- 新发现（待修复后再完成 review）：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 `SuspendCallAnalysis::handle_may_suspend_outward(...)` 当前会把借来的 `ProgramFacts` 整体 `clone` 后重新包成 `Rc` 再传给 `HandlePlanContext`；
  - 这会让 nested-handle suspendability 分析退回到“复制 side table”而不是“共享 side table”，与 `T5000c1R` 的 review 目标不完全一致，也会给热点分析路径增加不必要的固定成本。
- 已完成：局部收口 `SuspendCallAnalysis` 的 `ProgramFacts` 持有方式。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 `SuspendCallAnalysis` 已改为持有共享 `Rc<ProgramFacts>`；
  - nested `HandlePlanContext`、known-fun suspendability 分析路径以及 codegen 内 higher-order suspendability 查询路径均已改为传递 `Rc::clone(...)`，不再复制整表。
- 已完成：同步修复受该签名变化影响的测试 helper。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
- 已完成：验证修复结果。
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上均已通过。
- 已完成：回写 `TODO.md` 与 `PLAN.md`，将 `T5000c1R` 标记为完成，并记录 review 结论与修复项。
- 进行中：检查工作区、准备提交本轮变更。
