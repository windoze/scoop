# 执行计划

## 说明

用户要求先把思路与执行计划写入本文件，再执行仓库检查与代码修改。我会在不泄露内部完整推理细节的前提下，记录可审计的执行依据、假设、步骤和进度更新。

## 初始目标

本次只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有任务拆分、依赖与优先级。
4. 若首个未完成任务过大或存在未满足前置条件，则先在 `PLAN.md` / `TODO.md` 中重排或拆分任务，再执行本轮应该处理的第一个子任务。
5. 阅读与该任务直接相关的源码、测试和规范文件，确认当前实现状态。
6. 实现任务，避免使用绕过方案；如果遇到规范缺口或实现边界，先把真实缺陷补入 `TODO.md` 并调整依赖顺序。
7. 运行相关测试；必要时补充或修正测试，并处理编译、clippy、格式化等质量问题。
8. 更新 `TODO.md` 和 `PLAN.md`，记录完成情况或阻塞原因。
9. 提交 git commit，提交信息与任务编号/内容对应。
10. 停止，不继续下一个任务。

## 执行原则

- 不把变通方案当作完成。
- 如果发现规范不匹配、缺失功能或现有 bug，会先把它们转化为前置任务并调整任务顺序。
- 不回退或覆盖与当前任务无关的已有修改。
- 在关键节点更新本文件，便于跟踪进度。

## 进度记录

- 2026-04-14：已写入初始执行计划，下一步开始检查最新提交与任务列表。
- 2026-04-14：已检查最新提交 `63078b233ea01aec6ef31980380aa679addd4301`，提交说明未额外声明需要先修复的遗留问题。
- 2026-04-14：已定位首个未完成任务为 `T2003r3d2a`：补齐 unified resuming 的 plan-owned metadata 与 resolver helper。
- 2026-04-14：下一步读取 `T2003r3d2a` 的任务描述，并审查 `state_machine_plan.rs`、`shared.rs` 以及 unified resuming 相关 helper 的当前缺口，判断是否可在本轮直接完成。
- 2026-04-14：已确认 `T2003r3d2a` 仍可在本轮直接完成，不需要再拆分 `TODO.md` / `PLAN.md`。
- 2026-04-14：实现方案收敛为三部分：
  1. 在 `state_machine_plan.rs` 为 `FrameSlot` 补足局部元数据，并实现 `record_stmt_reads` / `record_expr_reads`。
  2. 在 effect shared metadata helper 中恢复基于 `SuspendSourcePath` 的 plan-driven resolver，并让 `collect_escape_capture_metas_from_plan` 只消费 unified plan 元数据。
  3. 补回定向测试：覆盖 read tracking、escape capture metadata、single/mixed direct+indirect resolver 的代表性路径；完成后只跑最小测试集与 `clippy`。
- 2026-04-14：接手继续执行。当前优先级：
  1. 整理 `state_machine_plan_tests.rs` 底部辅助函数，补齐 `lower_typed_single_source_with_source`、`find_handle_local_id_by_name` 及相关 AST 搜索 helper。
  2. 重新编译 `resolve_` / `plan_dump_` 定向测试，修复 shared metadata helper 与测试之间的签名或类型不一致。
  3. 通过最小定向测试后运行 `cargo clippy --workspace --all-targets -- -D warnings`。
  4. 若全部通过，再更新 `TODO.md`、`PLAN.md`、本文件并提交 `[T2003r3d2a] ...`，随后停止。
- 2026-04-14：`state_machine_plan_tests.rs` 底部辅助函数已补齐；`cargo test -p scoopc resolve_ --no-run` 与 `cargo test -p scoopc plan_dump_ --no-run` 已通过，说明 unified resolver/helper 的测试接口面已收齐。
- 2026-04-14：最小定向验证已通过：
  - `cargo test -p scoopc resolve_ -- --nocapture`
  - `cargo test -p scoopc plan_dump_ -- --nocapture`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 2026-04-14：已执行 `cargo fmt --all`，并确认 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` / `shared.rs` 中与本任务直接相关的 `todo!` / `unimplemented!` 已清空。
- 2026-04-14：本轮任务 `T2003r3d2a` 已完成，已同步更新 `TODO.md` / `PLAN.md`。下一步应转向 `T2003r3d2b`，但按要求本轮会在提交后停止。
