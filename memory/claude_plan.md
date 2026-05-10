# 执行计划

说明：按安全与协作要求，这里记录可执行计划、关键依据与进度，不记录不可审计的内部推理细节。

1. 先读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务；必要时查看最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题。
2. 阅读该任务条目中的要求、依赖、验证标准与完成记录，再只检查完成该任务所需的最小相关文件范围。
3. 如任务可直接实施，则进行最小正确修改；如遇到阻塞当前任务的真实缺陷或缺失能力，则先在 `TODO.md` 中按依赖顺序补入最小前置任务并停止在该点。
4. 运行与该任务直接相关的验证；若任务要求或改动范围需要，则补跑更广的测试与质量检查，直到结果稳定。
5. 更新 `memory/claude_plan.md` 记录进展；将任务在 `TODO.md` 中标记为 `[DONE]` 并填写完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
6. 按仓库约定创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度

- 已创建本计划文件。
- 已读取 `TODO.md`，定位首个未完成任务为 `G4-T05R`：Review ordinary callee suspend/reentry，确认 facts 驱动且无 TLS 旁路。
- 已检查最近一次提交：`[G4-T05] Rebuild ordinary callee suspend/reentry`，提交正文未额外记录与 `G4-T05R` 直接相关的未完成问题。
- 已完成最小范围审阅：`ordinary_callee.rs`、`mod.rs`、`closure/mod.rs`、`control_flow.rs`、`stmt.rs`、`effect_lowered/{body,layout,value}.rs`。
- 已完成验证：
  - `grep __scoop_callee_suspend_state|scoop_callee_suspend_state_|publish_incoming_resume_token|clear_incoming_resume_token crates/scoopc/src`：无命中。
  - `glob crates/scoopc/src/llvm/codegen/**/ordinary_callee.rs`：仅剩 neutral `llvm/codegen/ordinary_callee.rs`。
  - `cargo check -p scoopc`：失败前沿已停在后续 G6/G7 与 ordinary effect propagation 缺口，未回退到 G4 helper 或 deleted callee TLS bridge。
- review 结论：当前 ordinary callee `needs_reentry` shell 判定继续由 published callable facts 决定；resumed path 只从显式 `incoming_resume_token_ref` 恢复 suspend-state；无需额外源码修补。
- 已更新 `TODO.md`，将 `G4-T05R` 标记为 `[DONE]` 并补写完成记录。
- 下一步：检查工作区差异，创建本次任务提交，然后停止。
