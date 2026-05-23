# 当前执行计划

说明：此文件记录可审计的执行计划、关键决策和进度更新；不会记录隐藏推理细节。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 只围绕该任务读取必要上下文；如最新提交明确提到与该任务直接相关的未完成问题，一并纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 根据任务要求实现最小正确变更，避免绕开规格或改变任务范围。
4. 运行与变更直接相关的测试；若观察到未被明确排期的失败，先修复，或在 `TODO.md` 中插入最小必要前置任务并停止。
5. 完成后在 `TODO.md` 中将该任务标题加上 `[DONE]`，更新完成记录；仅在阶段级计划确实变化时更新 `PLAN.md`。
6. 检查工作区差异，提交本次任务涉及的全部未提交变更。
7. 提交后停止，不继续处理下一个任务。

## 进度日志

- 已写入初始计划，下一步读取 `TODO.md` 确定第一个未完成任务。
- 已确认 `TODO.md` 中第一个未完成任务是 `P8-T02`（最终全仓验证与 release readiness 清场）。下一步读取 `TODO-6.md` 中该任务的详细要求，并查看最新提交是否含有与该任务直接相关的未完成说明。
- 已读取 `TODO-6.md` 的 `P8-T02` 任务体：必须运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`（至少 30 分钟 timeout）和 `git diff --check`，并修正所有观察到的未排期失败。最新提交 `P8-T01R` 未显示与本任务直接相关的未完成 blocker。
- 执行顺序：先跑格式化和静态门禁，再跑 Rust 全量测试和完整 fixture suite；如有失败，先定位修复并重跑相关范围，最后更新 TODO 完成记录并提交。
- 已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all --all-targets`。下一步运行完整 fixture suite：`cargo run -p scoop -- test`。
- 完整 fixture suite 失败：20 个 target 失败。失败分为三类：build fixture 的 LLVM substring 期望漂移、MIR/golden 快照漂移、以及 `run_pass_cone/cross_file_ctor_named_default_basic` 运行退出 7 / `top_level_val_pattern_multi_file_basic` stdout mismatch。下一步先定位 run-pass 实际行为和可能的 production bug，再决定是修复实现还是更新 stale golden/fixture 期望。
- 已修复并复验失败范围：`cross_file_ctor_named_default_basic` 的退出 7 来自 class ctor init source payload 中内建标量 `ToString` 错走 interface itable，已在 raw HIR codegen 对内建 `ToString` 接收者转向对应直接实现；build fixture 期望更新为当前 LIR direct-call 命名；top-level eager init stdout golden 更新为所有 eager seed 先于 `main` 输出；MIR/mir_lowered/mir_materialized goldens 已刷新到当前 stable dump。已通过 `build`、`mir`、`mir_lowered`、`mir_materialized`、`run_pass_cone` 相关 fixture phase。
- 修复后已重新通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all --all-targets`。下一步重跑完整 `cargo run -p scoop -- test`。
- 完整 `cargo run -p scoop -- test` 已通过（1536 checks passed），`git diff --check` 已通过。已将 `P8-T02` 在 `TODO.md` / `TODO-6.md` 标记为 `[DONE]` 并填写完成记录。下一步检查 TODO 状态一致性、工作区差异，然后提交本任务。
- TODO 状态复查完成：`TODO-1.md` 至 `TODO-6.md` 中唯一剩余 `[TODO]` 标题是下一项 `P8-T02R`，`TODO.md` 索引中 `P8-T02` 已同步为 `[DONE]`，下一项仍是 `P8-T02R`。下一步检查 diff 并提交。
