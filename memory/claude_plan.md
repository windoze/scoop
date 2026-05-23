# 当前执行计划

说明：此文件记录可审计的执行计划、关键决策和进度更新；不会记录隐藏推理细节。

## 初始计划

1. 已按 `TODO.md` 索引确认第一个未完成任务是 `P8-T02R：Review final verification 与 release readiness`，对应详情位于 `TODO-6.md`。
2. 读取 `TODO-6.md` 的 `P8-T02R` 任务体并确认要求：复核 `P8-T02` 的最终验证完整性、P0-P8 是否仍有未完成任务或未跟踪 blocker、release tag 前是否还有必须修复的工作区/文档/测试风险。
3. 先查看最新提交和工作区状态，判断是否有与 `P8-T02R` 直接相关的未完成说明或前次中断遗留变更；若存在，纳入本 review。
4. 执行 review 复查：检查 `TODO.md` 与 `TODO-1.md` 至 `TODO-6.md` 的 `[DONE]`/`[TODO]` 状态一致性；复查关键 residual 搜索和 `dependency_gate` 覆盖是否仍满足 P0-P8 硬约束。
5. 按任务要求重新运行 `P8-T02` 验证矩阵；若成本过高则至少重跑与最近修复相关的范围并明确引用最近一次完整通过记录。本次优先尝试完整矩阵：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`（至少 30 分钟 timeout）、`git diff --check`。
6. 若任何测试/fixture 失败且未被明确排期，必须在本任务内修复并重跑对应范围；不能以历史噪声忽略。若发现会阻塞 `P8-T02R` 的真实前置问题且无法在本轮直接修复，则在 `TODO.md`/`TODO-6.md` 插入最小前置任务并提交后停止。
7. 若 review 通过，将 `P8-T02R` 在 `TODO.md` 和 `TODO-6.md` 同步标记为 `[DONE]`，填写 completion record，说明项目实现完成并可进入最终 release tag 流程；仅当阶段级计划改变时才更新 `PLAN.md`。
8. 最后检查差异与工作区，提交本任务所有相关变更，提交后停止，不进入 `TODO-7-INIT`。

## 进度日志

- 已确认本轮第一个未完成任务为 `P8-T02R`，并写入本次 review 执行计划。下一步查看最新提交、工作区状态和任务状态一致性。
- 最新提交为 `800e1707 [P8-T02] Final verification cleanup`，未显示与 `P8-T02R` 直接相关的未完成 blocker。当前工作区除本计划文件外还有未跟踪 `PLUGIN_ABI.md`；暂按非本任务改动处理，不回退、不纳入本任务提交，除非后续确认它与 release-readiness review 直接相关。
- TODO 状态复查：`TODO-6.md` 中唯一 `[TODO]` 标题是当前 `P8-T02R`，`TODO-7.md` 从 `TODO-7-INIT` 开始仍为后续任务；`TODO.md` 索引同步显示 `P8-T02R` 为第一个 `[TODO]`。未发现完成记录已填写但标题未 `[DONE]` 的 P0-P8 任务。
- residual 复查初步结论：旧 `comptime` / `const_eval` surface 在 `crates/scoopc/src` 无命中；LLVM production residual 搜索仅见 `llvm/frontend.rs` 的 single-file frontend preparation handoff 和 layout 测试中的 `materialized_pass_view`，与 `P8-T01R` 记录一致；`pipeline/llvm_codegen_stage.rs` 中 `HirFacts` / `MaterializedMir` / `MaterializedEffectFacts` 命中仍位于构造显式 `LlvmStageBaseContext` 窄合同的路径。
- 验证进度：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all --all-targets` 已通过。下一步运行完整 `cargo run -p scoop -- test`。
- 完整 `cargo run -p scoop -- test` 已通过（fixtures: ok，1536 checks）。`git diff --check` 已通过。下一步更新 `TODO.md` / `TODO-6.md` 的 `P8-T02R` 完成状态与记录。
- 已将 `P8-T02R` 在 `TODO.md` 和 `TODO-6.md` 同步标记为 `[DONE]` 并填写 completion record；`TODO.md` 当前状态更新为 P0-P8 已完成，下一项为 `TODO-7-INIT`。复查后 `TODO-1.md` 至 `TODO-6.md` 已无 `[TODO]` 标题，`TODO-7-INIT` 是唯一剩余首个 TODO；更新后的 `git diff --check` 通过。
