# 执行计划

## 当前目标

按 `TODO.md` 的优先级完成第一个未完成任务，然后停止；在执行过程中先检查最新提交是否暴露已有问题，若有则优先修复。

## 执行原则

- 只处理第一个未完成任务，完成并提交后停止。
- 若最新提交、探查、测试或实现过程中发现已有 bug、回归、规格不匹配、未完成边界或绕过实现，先修复该问题；如果不能立即修复，则把它作为前置任务插入 `TODO.md` 并提交后停止。
- 不使用 fixture-only hack、弱化测试、改变建模方式或规避语言/运行时缺口的方式完成任务。
- 所有计划变化和关键进度都会继续更新本文件。
- 本文件记录的是可公开的执行计划和决策摘要，不记录隐藏推理细节。

## 初始步骤

1. 查看工作区状态，确认是否有未提交的既有改动，避免覆盖用户改动。
2. 查看最新提交信息和变更，判断是否提到或暗示已有问题；如有，先修复。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读必要上下文，包括 `PLAN.md`、相关源码、测试和规格文档。
5. 如果第一个任务过大，拆解为更小子任务，更新 `TODO.md` 和 `PLAN.md`，提交拆解结果后停止。
6. 若任务可执行，按现有架构实现最小完整变更。
7. 添加或更新相关测试，运行必要测试命令；若出现问题，按项目规则修复或记录为前置任务。
8. 完成后更新 `TODO.md`、`PLAN.md` 和本文件，标记本次任务完成。
9. 运行最终验证，确保没有可见编译/测试问题。
10. 使用清晰的任务编号或描述提交变更。

## 当前进度

- 已创建本执行计划文件，下一步开始检查 Git 状态、最新提交和 `TODO.md`。
- 已检查工作区状态：当前只有本轮更新的 `memory/claude_plan.md` 处于修改状态。
- 已检查最新提交：`[T5000i3] Connect continuation escape facts to effect planning`。
  - 提交记录中提到的前置缺口是 handle MIR 结构占位导致 continuation escape facts 错误降级为 `Unknown`，该缺口已在该提交内修复。
  - 未发现最新提交说明中另有尚未修复、必须先于当前任务处理的独立问题。
- 已定位 `TODO.md` 中第一个显式未完成任务：`T5000i4 迁移 state_machine_plan / segments / transform 到 MIR + shared facts 边界`。
- 下一步读取 `T5000i4` 任务说明、`PLAN.md` 最新进度，以及当前 effect/state-machine planning、segments、transform 与 MIR/shared facts 的源码边界。
- 已完成边界核对：
  - planning 主体当前实际在 crate-root 文件 `effect_state_machine_analysis.rs`，但 LLVM backend 仍通过 `llvm/codegen/effect/state_machine_plan.rs` wrapper 和 `include!` 把 plan/segments/transform 主体编入 `unified_state_machine_skeleton`；
  - `segments` 与 `transform` 文件仍位于 `llvm/codegen/effect/`，且 emitter 从 backend-local skeleton 模块导入 contract/types；
  - `effect_state_machine_analysis.rs` 末尾还包含一整段 `MainCodegen` impl，导致 shared analysis 文件仍承担 backend 桥接职责。
- 当前实施方案：
  1. 新增 crate-root `effect_state_machine` shared 模块，统一承载 plan / segments / transform skeleton；
  2. 将 `state_machine_segments.rs` 与 `state_machine_transform.rs` 从 `llvm/codegen/effect/` 移到 crate-root shared 文件；
  3. 删除 LLVM-local `state_machine_plan.rs` wrapper 和 `unified_state_machine_skeleton include!`；
  4. 把 `MainCodegen` 专属桥接方法迁到 `llvm/codegen/effect/state_machine_bridge.rs`；
  5. 暴露 shared contract builder / ordinary callee suspend-plan helper，让 backend 只消费 shared 输出和 emitter contract；
  6. 更新 `effect_step_summary.rs` 直接复用 shared 模块，避免 no-LLVM 路径继续单独 include analysis 文件。
- 已完成初始迁移改动：
  - 新增 `crates/scoopc/src/effect_state_machine.rs` shared 模块；
  - `state_machine_segments.rs` / `state_machine_transform.rs` 已从 `llvm/codegen/effect/` 移到 crate root；
  - LLVM-local `state_machine_plan.rs` wrapper 已删除，`effect/mod.rs` 不再 include plan/segments/transform；
  - `MainCodegen` 的 effect-analysis context、known-fun suspendability cache、ordinary callee plan 和 unified contract bridge 已迁到 `llvm/codegen/effect/state_machine_bridge.rs`；
  - `state_machine_emitter.rs` 现在直接从 `crate::effect_state_machine` 导入统一 contract 和状态机类型；
  - `effect_step_summary.rs` 改为调用 `crate::effect_state_machine`，不再在 no-LLVM 路径单独 include analysis 文件。
- `cargo fmt --all --check` 首次只暴露格式化差异，已运行 `cargo fmt --all` 修正。
- 下一步运行编译/测试，处理迁移后的可见性、导入和 no-LLVM 边界问题。

## 2026-04-28 接手复核计划

- 先保持上一轮未提交改动，不回退、不重做；逐项检查当前 diff 是否完整对应 `T5000i4`。
- 读取 `T5000i4` 的任务说明、迁移后的 shared module、LLVM bridge/emitter、step summary 和 lib module 声明，确认职责边界。
- 运行最小编译检查，优先修复可见性、导入、cfg/no-LLVM 和移动文件后的路径问题。
- 若遇到上一轮提到的 server-side 卡点，先定位是否是测试/编译命令中的具体失败；能修复则直接修复，不能修复才按 `PROMPT.md` 插入前置任务并停止。
- 编译通过后运行相关测试，再按需要扩大到 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 完成后更新 `TODO.md`、`PLAN.md` 与本文件，提交 `[T5000i4] ...`，然后停止在本任务。

## 2026-04-28 组织结构调整

- 用户指出 shared effect/state-machine 代码继续散放在 `crates/scoopc/src/` 根目录下并不理想；该反馈与 `T5000i4` 的边界迁移目标一致，纳入本任务。
- 计划将 crate-root 的 effect 共享代码整理为目录模块：
  - `crates/scoopc/src/effect/mod.rs`
  - `crates/scoopc/src/effect/analysis.rs`
  - `crates/scoopc/src/effect/step_summary.rs`
  - `crates/scoopc/src/effect/state_machine/{mod.rs,analysis.rs,segments.rs,transform.rs}`
- 更新所有 `crate::effect_analysis`、`crate::effect_state_machine`、`crate::effect_step_summary` 调用点，避免保留根目录 shim 文件。
- 先完成机械迁移和导入修正，再运行格式化、编译、测试与 clippy。

## 2026-04-28 接手后的即时计划

- 已确认当前工作区就是上一轮 `T5000i4` 的未提交中间态，不包含新的用户改动；后续基于现有 diff 继续推进，不重置、不回退。
- 先核对最新提交 `dc23cf47 [T5000i3] Connect continuation escape facts to effect planning` 是否遗留必须先修的问题；目前仅看到本轮尚未完成的边界迁移，没有额外独立缺陷记录。
- 下一步顺序：
  1. 审核当前未提交 diff 与 `T5000i4` 验收条件的对应关系，找出缺口；
  2. 运行最小编译检查，定位真实阻塞点；
  3. 修正模块声明、可见性、导入和 no-LLVM 边界；
  4. 通过后运行针对性测试，再扩大到全量测试与 clippy；
  5. 更新 `TODO.md`、`PLAN.md`、本文件并提交 `[T5000i4] ...`。

## 2026-04-28 完成情况

- `T5000i4` 现已完成：shared effect/state-machine 代码已整理到 `crates/scoopc/src/effect/` 目录模块，`state_machine_plan / segments / transform` 的主分析入口不再依赖 LLVM backend-local wrapper。
- 已完成的结构边界收口：
  - 删除 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 backend-local `unified_state_machine_skeleton`；
  - 新增 `crates/scoopc/src/llvm/codegen/effect/state_machine_bridge.rs`，把 `MainCodegen` 专属桥接、known-fun suspendability cache 与 ordinary callee suspend-plan 构造从 shared analysis 文件中迁出；
  - `state_machine_emitter.rs` 与 no-LLVM `effect/step_summary.rs` 现在都直接复用 `crate::effect::state_machine` 的 shared contract / summary API。
- 已完成验证：
  - `cargo fmt --all --check`
  - `cargo test -p scoopc llvm::codegen::effect -- --nocapture`
  - `cargo test -p scoopc --no-default-features`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`（`fixtures: ok (1202)`）
- 下一步只剩按 `PROMPT.md` 提交本任务，然后停止。
