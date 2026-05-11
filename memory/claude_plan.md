# Claude Plan

## 约束说明

- 不写入或暴露内部逐步思维细节；这里记录可审计的执行计划、关键决策、阻塞与进度。
- 本次调用只处理 `TODO.md` 中第一个未完成任务；完成后更新记录并提交，然后停止。

## 初始执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断完成状态，定位第一个未完成任务。
2. 查看最近提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务范围或其前置依赖。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证要求与完成记录；必要时对相关实现文件做定向阅读。
4. 在不偏离规格、不采用权宜方案的前提下完成该任务；若遇到阻塞，新增最小前置任务到 `TODO.md` 并调整依赖顺序。
5. 运行该任务要求的验证，以及必要的回归检查；若失败则立即修复并重跑。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 更新 `TODO.md`：仅在任务真正完成时给任务标题加上 `[DONE]` 并填写完成记录；若只是阻塞，则保持未完成并记录新增前置任务。
8. 仅当阶段级计划或依赖结构发生变化时更新 `PLAN.md`。
9. 按仓库约定创建一次 git 提交，提交当前任务涉及的全部未提交变更，然后停止。

## 进度记录

- 已创建初始计划，下一步读取 `TODO.md` 与最近提交以确定当前任务。
- 已确认首个未完成任务为 `G8-T09R：Review 最终收口结果，确认仓库重新只剩 target-shape 单主线`。
- 最近提交为 `[G8-T09] Restore full target-shape regression coverage`，提交信息未额外声明需要先行插入的新未完前置问题。

## 当前任务执行计划（G8-T09R）

1. 复读 `G8-T09R` 条目，按其要求检查 runtime、backend、pipeline、sysroot、活跃测试/fixture/文档。
2. 对活跃代码目录执行定向 grep，确认 deleted TLS continuation/effect 名字未回流。
3. 复跑 `G8-T09` 记录中的完整验证矩阵，确认工作区仍满足最终收口状态。
4. 对关键目录做人工抽查，确认 runtime 只剩 generic substrate、backend 拥有 whole-function protocol、优化级别共用单一管线。
5. 若复核通过，更新 `TODO.md` 将 `G8-T09R` 标记为 `[DONE]`，补全完成记录并给出与 `EFFECT_REFACTOR_GAPS.md` 的最终对应关系。
6. 若全部任务完成且仓库状态允许，执行最终收尾提交，并创建 `v0.1.0` 标签。

## 阶段性进展

- 已查看最近提交 `[G8-T09] Restore full target-shape regression coverage` 的改动范围，确认它正对应 `G8-T09` 的实现面。
- 已对 `crates/scoopc/src`、`runtime/c`、`sysroot`、活跃测试/fixture/文档范围执行旧 TLS/bridge 名字扫描；未发现 `scoop_effect_handler_stack_top`、`scoop_continuation_resume_into`、`scoop_effect_outcome_publish`、`effect_call_wrapper` 等 deleted surface 回流。
- 当前工作区仅有 `memory/claude_plan.md` 未提交变更，尚未发现其他意外脏文件。
- 复跑验证矩阵时发现 `cargo fmt --check` 失败：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 中存在一处未按 rustfmt 折行的 `matches!` 条件。该问题直接否定“最终收口已完全通过验证矩阵”的结论，因此在本 review 任务内按最小改动修复。
- 下一步：从 `cargo fmt --check` 重新开始复跑完整验证矩阵，并抽查关键实现文件，形成最终 review 结论。

## 最终进展

- 已修复 `cargo fmt --check` 暴露的唯一格式回归。
- 已复跑并通过完整验证矩阵：`cargo fmt --check`、`cargo check -p scoop_runtime`、`cargo check -p scoopc`、`cargo test -p scoop_runtime`、`cargo test -p scoopc`、`cargo test -p scoop`、`cargo test --all`、`cargo clippy --workspace --all-targets -- -D warnings`，以及两条定向回归。
- 已完成人工复核：runtime internal TLS / exported ABI 不再承载 continuation/effect policy；backend 仍以显式 hidden ABI + `EffectCtx` / `EffectOutcome` / generated continuation driver 作为 authoritative protocol；session/pipeline/CLI 测试继续锁定单一 target-shape 管线。
- 已更新 `TODO.md`，将 `G8-T09R` 标记为 `[DONE]`，并写入最终 review 结论与 1-12 全部 gap 的最终对应关系。
- 下一步：检查待提交变更，创建最终提交，并打 `v0.1.0` 标签。
