# 当前执行计划

## 当前状态

- 首个未完成任务已确认为 `P3-T07`：P3 全包清场、文档同步与依赖审计。
- 最新提交为 `f1b6795b [P3-T06R] Review dispatch devirtualization owner migration`，它直接作为 `P3-T07` 的前置 review，不包含额外未完成前置项。
- 本文件记录可公开的执行计划与进度；内部推理不写入文件。

## 步骤计划

1. 用定向搜索审计 P3 边界：`MirStageOutput`、旧 root inventory map、`materialized_pass_view()`、`MaterializedMirPassView`、`MaterializedMirPassArtifacts`、`try_devirtualize_dispatch_target(...)`、`devirtualize_dispatch_calls`。
2. 将命中分类为已清理、P3 合法 owner、P4/P5/P7 过渡残留或测试命中。
3. 检查 `scoopc_mir_facts`、MIR pass pipeline、README 与 dependency gate 文档是否准确可见。
4. 只做必要的文档、元数据和测试期望修正；阶段状态确实变化时才更新 `PLAN.md`。
5. 运行 `P3-T07` 要求的验证命令，并修复范围内失败。
6. 在 `TODO.md` 与 `TODO-4.md` 中把 `P3-T07` 标记为 `[DONE]`，填写审计、验证和残余风险记录。
7. 检查 `git status`、`git diff` 与最近提交，提交本任务变更。
8. 提交后停止，不进入下一个任务。

## 进度记录

- 已记录初始计划。
- 已定位当前任务为 `P3-T07`，并围绕审计、文档同步和验证要求细化执行计划。
- 已完成 P3 边界定向搜索：活跃 HIR 源码无 materialized MIR/pass-view 或去虚化 owner 残留；活跃 Rust 源码无旧 root inventory map 字段。
- 已更新 `README.md`、`PIPELINE-CLEANUP.md`、`PIPELINE_REFACTOR.md`、`PLAN.md`、dependency gate 帮助/文档，准确记录 P3 完成状态和 P4/P5/P7 残余边界。
- `cargo test -p scoopc --no-default-features mir_stage` 暴露了 P3 测试仍按旧 3-step pass pipeline 断言；已修正为断言包含 devirtualization 的显式 4-step MIR pass 顺序。
- `P3-T07` 验证已全部通过：`cargo fmt`、`cargo test -p scoopc_mir_facts`、`cargo test -p scoopc --no-default-features mir_stage`、`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo test -p scoopc --no-default-features effect_lowering_stage`、`cargo run -p scoop_tools -- dependency-gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 最终审计搜索确认：无活跃 HIR materialized MIR/pass-view 残留，无 HIR devirtualization owner，无旧 root inventory map 字段；`try_devirtualize_dispatch_target(` 的非测试活跃命中只剩 MIR pass/shared helper 与 P7 backend residual。
- 已在 `TODO.md` 和 `TODO-4.md` 中把 `P3-T07` 标记为 `[DONE]`，完成记录包含审计结果、验证命令和 P4/P5/P7 残余风险。
