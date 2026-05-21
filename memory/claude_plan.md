# 当前执行计划

本文件记录本次调用的可执行计划和进度。这里记录的是实施步骤与决策依据摘要，不包含不可见的内部推理。

## 计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 标记的任务。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如有，将其作为当前任务范围或必要前置项处理。
3. 阅读当前任务所需的相关代码、测试、文档和约束，避免进行与当前任务无关的开放式历史问题清扫。
4. 完整实现当前任务；如果发现阻塞当前任务的真实前置缺口，则只新增最小必要的前置任务并停止。
5. 运行任务要求的验证命令和相关测试；若发现当前改动引入的问题，立即修复并复测。
6. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]`，并填写完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
7. 检查工作树差异，提交本次任务涉及的所有必要文件，提交信息使用任务编号和清晰描述。
8. 完成一个任务后停止，不继续处理后续任务。

## 进度

- 已创建初始执行计划。
- 已读取 `TODO.md`，确认当前第一个未完成任务为 `P2-T01R`。
- 已读取 `TODO-3.md` 中 `P2-T01R` 细节；本次任务是 review `scoopc_hir_facts` crate、事实模型和 dependency gate，不进入 `P2-T02`。
- 最近提交为 `[P2-T01] Add HIR facts crate skeleton`，与当前 review 直接相关，未发现提交信息中声明的额外未完成问题。
- 已完成静态复查：`scoopc_hir_facts` 的直接依赖为基础 crate，`HirFacts` 已按 declaration/source-site/global/native/type-context 分组，`scoopc` 仅提供迁移期 re-export anchor，dependency gate 已包含 fact crate 分类和对应单元测试。
- 下一步运行 `P2-T01R` 指定验证命令；如有失败，先修复同一 review 范围内的问题。
- 验证已通过：`cargo fmt`、`cargo check -p scoopc_hir_facts`、`cargo test -p scoopc_hir_facts`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoop_tools dependency_gate`、`cargo tree -p scoopc_hir_facts`、`cargo clippy --all-targets -- -D warnings`。
- Review 结论：未发现阻塞项；下一步只更新任务完成记录并提交。
- 已将 `P2-T01R` 在 `TODO.md` 与 `TODO-3.md` 标记为 `[DONE]`，并写入 review 完成记录、验证命令和残余风险。
- 下一步执行提交前差异检查并创建本任务提交。
