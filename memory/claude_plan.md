# 执行计划

本文件记录当前调用的可检查执行计划与进度。不会记录内部推理细节。

## 计划

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查最新提交与该任务是否存在直接相关的未完成事项。
3. 阅读该任务相关代码、测试与规范，确定最小正确实现范围。
4. 实现首个未完成任务；如发现阻塞该任务的具体前置缺口，按要求更新 `TODO.md` 并停止。
5. 运行该任务要求的验证命令和必要的回归测试，修复失败项。
6. 在 `TODO.md` 中给完成任务标题加 `[DONE]`，更新完成记录。
7. 运行最终状态检查，提交本次任务涉及的全部更改。
8. 停止，不继续处理下一个任务。

## 当前进度

- 已读取 `TODO.md`，首个未完成任务为 `MIR-T13：收口 remaining MIR-facing frontend/runtime policy gates`。
- 最新提交为 `MIR-T12R` review，无直接相关未完成事项。
- 已定位 `MIR-T13` 相关实现：finally/ResumeUnwind pending completion contract 已存在但缺命名验证；GC pin/handle 作为 direct intrinsic call 进入 MIR 但缺少 MIR-owned policy metadata；cross-thread non-Pure continuation 需要明确 typecheck diagnostic。
- 当前实现计划：扩展 `CallTransportMetadata` 的 GC intrinsic policy contract；在 typecheck 中拒绝 cross-thread outward propagation；添加 `refactor_mir_policy_gates` 测试、diagnostics fixtures 与 preflight denylist 维护；运行定向验证与 lint。
- 已实现 `MIR-T13`：GC pin/handle call transport 发布 MIR-owned policy metadata；cross-thread non-Pure continuation 在 typecheck 阶段拒绝；ResumeUnwind/finally pending completion handoff 增加 `refactor_mir_policy_gates` 验证；or-pattern binder、cross-thread outward propagation、GC handle unsupported surface 均有 diagnostics fixture。
- 已通过验证：`cargo test -p scoopc --no-default-features refactor_mir_policy_gates`、`cargo test -p scoopc --no-default-features refactor_hir_preflight`、`cargo test -p scoopc --no-default-features refactor_materialized_mir`、`cargo test -p scoopc --no-default-features refactor_mir_no_todo`、三个 diagnostics fixtures、`dump-mir`/`dump-effect-lowered` handle policy smoke、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 已更新 `TODO.md` 完成记录，下一步提交本次任务全部更改。
