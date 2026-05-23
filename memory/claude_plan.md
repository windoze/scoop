本文件记录本次调用的可审计执行计划和进度。不会记录模型的隐藏推理细节。

## 初始计划

1. 读取 `TODO.md`，只识别第一个标题未带 `[DONE]` 的任务，不做开放式历史问题扫查。
2. 查看该任务必要的上下文、相关代码和最近提交，确认是否存在直接阻塞当前任务的未完成问题。
3. 按任务要求实现最小正确改动；如果遇到当前任务无法绕开的缺失特性或规格不匹配，则在 `TODO.md` 插入最小 prerequisite 任务并停止。
4. 运行与当前任务直接相关的测试；如发现未被明确排期的失败测试或 fixture，修复它或把最小修复任务加入 `TODO.md` 的正确位置。
5. 更新 `TODO.md`：完成时给当前任务标题加 `[DONE]` 并补全 completion record；只有阶段级计划变化才更新 `PLAN.md`。
6. 运行必要格式化或验证命令，检查工作区差异。
7. 提交本次任务相关的所有变更，然后停止，不继续下一个任务。

## 进度日志

- 已写入初始执行计划。
- 已读取 `TODO.md` 和 `TODO-7.md`，确认本次唯一执行任务为 `P9-T01R：Review 后向边消除结果`。
- 最近提交为 `[P9-T01] Eliminate stage split back edges`，与当前 review 直接相关；下一步复核该提交触碰的 base crate 迁移、残余搜索和验证命令。
- 工作区另有未跟踪 `PLUGIN_ABI.md`，当前判断与本任务无关；不会修改或提交。
- Review 搜索发现 `typecheck/lower.rs` 仍通过 `crate::hir::EFFECT_ROW_PARAM_DECL_FILE` 访问 HIR-owned 常量；已把该 effect-row 参数标记下沉到 `scoopc_types`，并同步修正 typecheck/MIR 调用点。
- 已补强 `dependency_gate` source-tree 规则，覆盖 HIR 直接 MIR residual 与 typecheck 直接 HIR residual，避免同类后向边回归。
- 验证已通过：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop_tools -- dependency-gate`、P9-T01/P9-T01R 残余搜索、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 已将 `TODO.md` 与 `TODO-7.md` 中 `P9-T01R` 标记为 `[DONE]`，并写入 review 修正和验证记录。
