# 执行计划与进度记录

## 说明

按用户要求，本文件在任何仓库检查或代码执行前创建。
我不会记录内部逐词思维过程，但会记录足够详细的执行计划、决策摘要、关键发现、变更步骤与进度更新，便于审查。

## 当前任务总流程

1. 检查最新一次提交，确认提交说明中是否提到了已知问题、遗留问题或待修复项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划、依赖关系和任务顺序是否与 `TODO.md` 一致。
4. 若第一个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次仅执行拆分后的第一个子任务。
5. 实现本次应执行的任务。
6. 运行相关测试、格式化、lint；若发现失败或规范偏差，优先修复。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞原因。
8. 提交本次修改，随后停止，不进入下一个任务。

## 初始判断约束

- 只完成一个任务或一个新拆分出的首个子任务。
- 如果发现规范不匹配、实现缺口或现有 bug 阻塞当前任务，必须先把该问题写入 `TODO.md` 并调整顺序，不能通过绕过方式继续。
- 若最新提交中明确提到未解决问题，则这些问题优先于普通任务处理。
- 需要尽量做到无编译警告，并运行合适的验证命令。

## 进度日志

- 2026-04-18：已创建本文件并写入初始执行计划。下一步将检查最新提交与任务列表。
- 2026-04-18：已检查最新提交 `5394f81 [T3009b2cR] Review multi-site ordinary callee resume dispatch`。提交正文未声明新的待修遗留问题。
- 2026-04-18：已读取 `TODO.md` / `PLAN.md`。当前第一个未完成任务是 `T3009b2R`：复审“间接 callee resumed-body caller-tail 已统一接回”。
- 2026-04-18：判断 `T3009b2R` 属于可直接执行的复审任务，暂不需要先拆子任务。下一步将审查以下生产代码与验证链：
  1. `crates/scoopc/src/llvm/codegen/mod.rs` 中 ordinary callee suspend/resume 入口。
  2. `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中 callee suspend-state 保存/恢复。
  3. `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中 unified contract 对 suspend site 的建模。
  4. 与 indirect callee / multi-site / statement-container 相关的定向测试与 run-pass fixture。
- 2026-04-18：已完成首轮代码复审，当前结论摘要如下：
  - `build_ordinary_callee_suspend_plan_from_unified_contract()` 不再有 single-site 前提；它会遍历全部 `builder.suspend_sites`，为每个 `Perform` site 生成 `CalleeSuspendResumeSite`，并用 union locals 构建统一 `CalleeSuspendPlan.saved_locals`。
  - `codegen_top_level_fun()` 与 `codegen_closure_fun_body()` 都通过 `build_*_callee_suspend_plan(...)` + `codegen_callee_resume_dispatch(...)` 接入同一 fresh/resume 双入口；未发现 top-level / closure / function-value 的分叉特判。
  - `emit_callee_suspend_state_save()` / `begin_callee_suspend_resume()` 已把 `site_tag + resume_word + resume_gc_ref + saved locals` 纳入同一 callee suspend-state 对象；resume 通过 `site_tag` switch 回到对应 `resume_site*` block。
  - `emit_resume_after_call_site()` 会优先检查是否存在 captured callee suspend-state；若存在，则把 resume payload 写回该 state 并重放原 call expr，让 callee 自己完成 resumed body，再把真实 call result 写回 frame slot，而不是把 payload 直接当成整次调用结果。
  - continuation/runtime 侧已存在 `captured_callee_suspend_state` 合同：continuation 捕获该 state，resume 时临时恢复到 TLS，step 返回后再恢复 caller TLS。
- 2026-04-18：未发现新的 fixture-name patch 或 branch-count/source-shape 特判回流到当前复审链路。下一步运行定向测试、全量 `cargo test --all` 和 `cargo clippy --all-targets -- -D warnings`。
- 2026-04-18：定向验证已通过：
  1. `cargo test -p scoopc ordinary_multi_site_callee_materializes_resume_site_dispatch -- --nocapture`
  2. `cargo test -p scoopc indirect_if_branch_callee_keeps_handle_call_site_active_dispatch -- --nocapture`
  3. `cargo test -p scoop_runtime continuation_resume_temporarily_restores_captured_callee_suspend_state -- --nocapture`
  4. 9 条 indirect-callee / multi-site / statement-container / payload 相关 run-pass fixture
- 2026-04-18：全量质量门槛已通过：
  1. `cargo test --all`
  2. `cargo clippy --all-targets -- -D warnings`
- 2026-04-18：已更新 `TODO.md` 与 `PLAN.md`，将 `T3009b2R` 标记为完成。当前下一项为 `T3009b`。接下来只需检查工作树并提交本轮结果。
- 2026-04-18：提交前检查完成。当前工作树仅包含 `TODO.md`、`PLAN.md` 与 `memory/claude_plan.md` 的本轮记录变更；准备提交 `[T3009b2R] Review indirect callee resumed-body caller-tail`，提交后停止。
