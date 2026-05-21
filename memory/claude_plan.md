# 当前执行计划

## 约束摘要

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只处理第一个标题未带 `[DONE]` 的任务，完成后停止。
- 如遇阻塞当前任务的实现缺口或规格不匹配，先在 `TODO.md` 中加入最小必要前置任务并提交，然后停止。
- 任务完成必须更新 `TODO.md` 标题与完成记录，并提交所有本次相关变更。
- 不做开放式历史问题扫描，不处理与当前任务无关的问题。

## 步骤计划

1. 读取 `TODO.md`，定位第一个未完成任务，并查看该任务的要求、依赖与验证命令。
2. 查看最近提交信息，仅判断是否有与当前任务直接相关的未完成事项。
3. 根据当前任务读取必要的源码、测试、规格或文档，确认实现边界。
4. 若任务可直接完成，实施最小正确修改；若存在必须先修复的阻塞问题，更新 `TODO.md` 记录前置任务并停止。
5. 运行任务要求的验证命令与必要的补充测试，修复发现的直接相关问题。
6. 更新 `TODO.md`，给完成任务标题加 `[DONE]`，补充完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff`、最近提交，确认只提交预期变更。
8. 以清晰任务编号提交本次改动，然后停止，不进入下一项任务。

## 当前进度

- 已创建本计划文件。
- 已读取 `TODO.md`，首个未完成任务为 `P3-T06R`。
- 最新提交为 `a538af92 [P3-T06] Move dispatch devirtualization to MIR pass`，与当前 review 任务直接相关。
- 已读取 `TODO-4.md` 中 `P3-T06R` 的具体验收要求。
- 初步复查结果：HIR lowering 不再存在 `devirtualize_dispatch_calls` 开关；MIR pass pipeline 调度 `MirPassKind::Devirtualization`；materialization rewrite 对 virtual/interface call 只做 receiver/type substitution、candidate discovery 和 canonical target 记录。
- 搜索结果：`devirtualize_dispatch_calls` 无命中；`try_devirtualize_dispatch_target(` 仅出现在 MIR pass、共享 helper，以及已明确归属 P7 的 LLVM reachability/codegen residual。
- 已运行并通过验证：`cargo fmt`、`cargo test -p scoopc --no-default-features hir`、`cargo test -p scoopc --no-default-features mir::materialize`、`cargo test -p scoopc --no-default-features monomorph`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 已将 `P3-T06R` 在 `TODO.md` 与 `TODO-4.md` 标记为 `[DONE]`，并填写 review 完成记录。
- 下一步检查最终 diff/status 并提交本次 review 任务。
