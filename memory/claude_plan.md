# 当前执行计划

## 约束
- 以 `TODO.md` 为唯一任务排序与完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 如遇到阻塞当前任务的实现缺口，不用 workaround；在 `TODO.md` 中新增最小必要前置任务并停止。
- 完成任务后更新 `TODO.md`、运行相关验证、提交 Git commit。

## 步骤
1. 读取 `TODO.md`，定位第一个未完成任务及其验证要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 读取当前任务涉及的代码、测试与文档，限定调查范围。
4. 实现任务或在发现硬性阻塞时更新 `TODO.md` 添加前置任务。
5. 运行任务要求的验证与必要的回归测试，修复当前任务引入的问题。
6. 更新 `TODO.md`：为完成任务标题加 `[DONE]` 并填写完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 更新本文件记录关键进度。
8. 提交所有与本次任务相关的变更，然后停止。

## 当前状态
- 已定位第一个未完成任务：`CG-T00：建立 codegen gap inventory 与 backend gate`。
- 最近提交 `ba02261c Update plan` 未直接指向 `CG-T00` 的未完成实现问题。
- 已确认现有后端入口主要在 `crates/scoopc/src/llvm/emit.rs` 与 `crates/scoopc/src/llvm/codegen/mir_body.rs`，refactor smoke 已集中在 `effect_refactor_pipeline/llvm_codegen_stage.rs` 与 `scoop` CLI 测试。
- 当前实施方案：新增 `llvm::codegen_gap_inventory` 静态 owner map；在 `codegen_top_level_mir_fun` 进入 raw MIR body emission 前调用 backend gate；补 `codegen_gap_inventory*` 与 `refactor_llvm_backend_gate*` 单测。
- 已完成实现：新增 inventory、`RefactorBackendGate` 诊断、raw MIR backend gate，并将 refactor LLVM smoke 测试纳入 `refactor_llvm_backend_gate` 过滤名称。
- 已完成验证：`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、inventory trigger 搜索、`cargo clippy --all-targets -- -D warnings`。
- 已更新 `TODO.md`，将 `CG-T00` 标记为 `[DONE]` 并写入完成记录。
- 下一步提交本次任务变更，然后停止。
