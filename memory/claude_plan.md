# 当前调用计划

## 范围

- 以 `TODO.md` 作为权威任务列表。
- 本次只处理第一个标题未带 `[DONE]` 的任务：`P5-T05R：Review P5 全包完成度`，完成后停止。
- 先按当前任务做定向复查，不做开放式历史问题扫查。
- 不使用规避方案、不弱化行为；若发现阻塞 P5-T05R 的具体前置项，则在 `TODO.md` 写入最小 prerequisite，提交并停止。

## 执行步骤

1. 检查最新提交标题和工作区状态，确认是否存在与 `P5-T05R` 直接相关的未完成事项或前次遗留改动。
2. 复读 `P5-T05R`、`P5-T05` 与相关文档要求，明确 review 必须覆盖的输出边界、facts/query layer、TODO-6 residual 和验证命令。
3. 定向搜索 `StageOutput` 嵌套上游整包、legacy `EffectLoweredStageOutput` public API、`EffectFactsStageOutput` / `LirStageOutput` 上游 accessor、`canonical_snapshot_mut()`、LIR opt 读取 HIR/MIR/effect solver 输入等残留。
4. 读取命中的关键实现与文档位置，判断是否满足 `EffectFactsStageOutput = { effect_facts }`、`LirStageOutput = { lir, lir_facts }`、LIR facts 作为 P7 backend cleanup 输入基础，以及 P6/P7/P8 residual 未被误标为完成。
5. 如果发现 review 范围内可直接修复的问题，进行最小完整修复并补充必要测试或文档；如果发现无法在本任务内完成的真实 prerequisite，则更新 `TODO.md` 依赖顺序并停止。
6. 运行 P5-T05 指定验证：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_effect_facts`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo test -p scoopc --no-default-features effect_lowered`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
7. 额外执行任务要求的 residual 搜索，并把搜索结论写入 `TODO-5.md` 的完成记录。
8. 将 `TODO-5.md` 中 `P5-T05R` 标为 `[DONE]`，并同步 `TODO.md` 索引状态；仅在 phase/stage 计划实际变化时更新 `PLAN.md`。
9. 检查 `git status`、`git diff`、`git log --oneline -10`，确认只提交本任务相关改动。
10. 用描述性提交信息提交本次变更，然后停止，不开始 `TODO-6-INIT`。

## 进度记录

- 已读取 `TODO.md` 并确认第一个未完成任务是 `P5-T05R`。
- 已读取 `TODO-5.md` 的 `P5-T05R` 任务正文和前置 `P5-T05` 完成记录。
- 最新提交为 `[P5-T05] Complete P5 cleanup audit`；除本次计划文件外起始工作区干净。
- 初步 review 搜索确认：`canonical_snapshot_mut(` 无匹配；`EffectFactsStageOutput` 只保存 effect facts；`LirStageOutput` 不保存上游 stage output wrapper；LIR opt 的上游输入搜索只命中测试 fixture imports。
- Review 修复：更新 `effect_lowered` 模块注释，避免把 LIR opt 描述成普通闭世界 devirt/inline；更新 `PIPELINE_REFACTOR.md`，把 P5 已完成边界和 TODO-6/P6-P8 residual 分开。
- 独立审计指出 `LlvmCodegenStageOutput` / `StageEmitInput` 仍传播 P5 handoff wrapper；该项属于 TODO-6/P7 backend cleanup residual，已同步写入 `PIPELINE-CLEANUP.md`、`PIPELINE_REFACTOR.md` 和 `TODO-6.md`，不作为 P5 输出边界阻塞项。
- 验证已通过：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_effect_facts`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo test -p scoopc --no-default-features effect_lowered`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 额外 residual 搜索已完成：`canonical_snapshot_mut(` 在 `crates/` 无匹配；P4/P5 output wrapper 搜索只命中 `EffectLoweringStageInput` 的并列 import / “不保存 wrapper”注释；LIR opt upstream input 搜索无生产命中。
- 已将 `TODO-5.md` 的 `P5-T05R` 标为 `[DONE]` 并填写完成记录；已同步 `TODO.md` 索引状态。
- 下一步：检查最终 diff/status/log 并提交本任务。
