# 执行计划

本文件记录当前 invocation 的可审计执行计划、进度、决策和验证结果；不记录私有推理细节。

## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 阅读该任务的详情、依赖和验证要求；仅在最新提交直接相关时检查是否有未完成事项。
3. 检查当前任务涉及的代码、fixture 和文档边界。
4. 按任务要求完整实现或复审，不用窄化 fixture、替代表示或 workaround 绕过问题。
5. 增加或更新聚焦测试，覆盖当前任务的行为边界。
6. 运行任务要求的验证命令和必要的扩展验证，发现阻塞问题则修复。
7. 更新 `TODO.md` / 对应 `TODO-N.md` 的任务状态和完成记录。
8. 更新本文件，记录关键进展、计划变化和验证结果。
9. 检查 `git status`、`git diff` 和最近提交，提交本任务相关变更。
10. 完成并提交一个任务后停止。

## 进度记录

- 已启动 invocation，并写入初始执行计划。
- 已读取 `TODO.md`；第一个未完成任务是 `P5-T03R`，本次只完成该 review 任务。
- 已读取 `TODO-5.md`；`P5-T03R` 要求复审 `P5-T03` 的 query 切换结果，重跑验证，搜索旧 `LirStageOutput` 公开 accessor，修复 review 中发现的阻塞项，然后只标记 `P5-T03R` 完成。
- 复查发现阻塞点：`llvm/codegen/effect_lowered/layout/carrier.rs` 的 Program ABI materialization 仍通过 codegen pass-view / HIR 查询 carrier fallback 与签名信息；必须迁到 LIR facts 或明确退化为 backend-only physical residual 后才能完成 review。
- 已修复阻塞点：`LirEffectStepCallableFacts` 现在发布 effect-step 参数类型与 closure-carrier 参数类型；carrier ABI layout 改为从这些 LIR facts 读取签名，不再通过 `materialized_pass_view()` / HIR `fun_index` 查询 carrier 签名或 plain fallback 分类。
- 验证结果：`cargo check -p scoopc --features llvm`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --features llvm effect_lowered`、`cargo test -p scoopc --features llvm llvm::tests::late_lower`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 均通过。`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 仍失败 7/415，失败类别与 `P5-T03` 已记录的既有 TODO-6/P7/frontend-runtime residual 一致。
- 已更新 `TODO.md` 和 `TODO-5.md`，将 `P5-T03R` 标记为 `[DONE]`，并记录 review 结论、阻塞修复、accessor 搜索、residual 隔离和验证结果。
