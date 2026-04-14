# 执行计划与进度记录

## 说明

本文件记录本次执行的高层计划、关键决策、阻塞原因与完成进度，便于审计与续接。  
不记录逐字的内部思维链路，但会持续更新可验证的执行步骤、发现与结论。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息里是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前任务顺序、依赖与项目阶段。
4. 如第一个未完成任务过大，则将其拆解为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，随后只执行拆解后的第一个子任务。
5. 实现当前应执行的任务，必要时补充或调整相关代码与测试。
6. 运行必要的验证，包括与改动相关的测试，以及项目要求的格式化、lint、无 warning 检查。
7. 更新 `TODO.md` 与 `PLAN.md`，记录任务完成情况、依赖变化或新发现的问题。
8. 提交本次改动，提交后停止，不继续处理下一个任务。

## 执行约束

- 一次调用只完成一个任务（或一个新拆出的首个子任务）。
- 若发现规范不匹配、缺失能力、运行时缺陷或任何不能以规范方式完成当前任务的问题：
  - 必须先在 `TODO.md` 中补充前置修复任务并重排顺序；
  - 在 `PLAN.md` 中记录阻塞原因；
  - 提交这些计划性调整后停止。
- 不以绕过方案、测试特判或临时兼容方式宣称任务完成。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新提交 `33846d30d90f4d7295566611366d9ae106c7addb`（`[T3001] Delete callee-suspend shape routes`）；提交信息未额外提到需要在本轮先修复的遗留问题。
- 已阅读 `TODO.md` 与 `PLAN.md`，确认首个未完成任务是 `T3001R`，且该任务属于定向 review，不需要进一步拆分。
- 已完成 `T3001R` 审查：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相关调用点中检索旧符号与等价变体，未发现 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 或换名保留的 callee-shape 主路径。
  - 已复查 `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call`、`ExprKind::Perform` / `ExprKind::Handle` 调用接线，确认顶层函数与 closure 只走常规 codegen；effect 相关表达式统一进入 `effect/mod.rs` 占位入口，没有按源码形状切换另一套 emitter。
  - 当前 `effect/mod.rs` 只保留统一 state-machine 骨架与“尚未重新接回 unified lowering”的占位报错；未发现旧 callee-suspend route 等价回流。
- 已完成验证：
  - `cargo check -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
- 下一步：更新 `TODO.md` / `PLAN.md` 记录 `T3001R` 完成状态，然后提交并停止。
