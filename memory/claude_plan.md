# 执行计划

## 当前目标
- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。

## 约束与执行原则
- 先检查最新提交是否提到已有问题；如果有，优先修复这些问题。
- 先读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 任何发现的规格不匹配、缺失特性或错误实现，都必须作为正式任务写回 `TODO.md`，调整依赖顺序，不能用变通方案跳过。
- 实现后必须运行相关测试，并尽量补充必要测试；同时确保 `cargo clippy --all-targets -- -D warnings` 无告警。
- 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次清晰的 Git commit，然后停止。

## 步骤计划
1. 查看最新一次提交，确认是否提到需要先处理的遗留问题。
2. 读取 `TODO.md`，找出第一个未完成任务。
3. 读取 `PLAN.md`、相关源码与测试，确认任务范围与依赖。
4. 如任务过大，先拆分并更新计划文件；否则直接开始实现。
5. 修改代码并补充/调整测试。
6. 运行格式化、相关测试、必要的全量检查与 `clippy`。
7. 更新 `TODO.md`、`PLAN.md`、本文件中的进度记录。
8. 提交 Git commit，停止本轮工作。

## 进度记录
- 已创建初始执行计划，尚未开始代码与仓库状态检查。
- 已检查最新提交 `44f2edb87e2cc102cfdff5f20cf08cc0d0299399`，提交主题为 `[T3009b0a1d] Fix ObjectInitAccess inactive continue path`，未发现额外的提交说明或明确列出的未修复遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，定位到当前第一个未完成任务为 `T3009b0a1dR`：复审 `ObjectInitAccessBoundary` 的 inactive-path 是否真正统一收口到 state-machine 合同。
- 已阅读关键生产代码：
  - `state_machine_emitter.rs` 中 `UnifiedStateTerminator::Suspend` 的共享 inactive/active 分流；
  - `state_machine_plan.rs` 中 object value / property access 到 `ObjectInitAccessBoundary` 的建模；
  - `mod.rs` 中 `codegen_object_value_access` / `codegen_object_property_access` 的普通 codegen 路径。
- 当前已核实的要点：
  - `SuspendSiteKind::ObjectInitAccess` 已进入共享 `suspend_site_uses_inactive_continue_path` 集合；
  - step function 生成期间会临时清空 `current_fun_return_ty` / `return_context`，因此普通 `emit_ordinary_call_effect_propagation_check` 不会在 unified state-machine 内提前返回，inactive/active 分流仍由共享 `Suspend` terminator 负责；
  - 目前尚未发现按 object 名称、属性名或源码形状决定 inactive-path 的新分流。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop` 通过，输出与 golden 一致；
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 本轮结论：
  - 未发现需要修复的新生产代码问题；
  - `ObjectInitAccessBoundary` 的 inactive-path 已统一收口到 state-machine 合同，没有回流为 object 专用 patch 或源码形状分流；
  - 当前下一任务已推进为 `T3009b0a1e`。
- 下一步：检查工作区差异，提交 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 的更新，然后停止本轮工作。
