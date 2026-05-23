# 当前执行计划

1. 读取 `TODO.md`，按文件顺序确认第一个标题未带 `[DONE]` 的任务。
2. 查看该任务的要求、依赖、验证方式和完成记录；必要时查看 `PLAN.md` 和相关源码以建立上下文。
3. 检查最新提交是否明确提到与当前任务直接相关的未完成问题；如果会阻塞当前任务，将其纳入当前任务或在 `TODO.md` 中补充为前置任务。
4. 实现第一个未完成任务；如果发现必须先修复的具体缺口，则只添加最小必要前置任务并停止。
5. 运行与当前任务相关的测试；若发现未被计划覆盖的失败测试或 fixture，先修复或在 `TODO.md` 中排到当前任务完成前。
6. 任务完成后更新 `TODO.md`：在任务标题加 `[DONE]`，并补全完成记录；仅当阶段计划确实改变时才更新 `PLAN.md`。
7. 检查工作区差异，提交所有与本次任务相关的改动，提交信息使用任务编号和简短说明。
8. 完成提交后停止，不继续处理下一个任务。

## 进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 确认第一个未完成任务。
- 已读取 `TODO.md` 与最新提交摘要；第一个未完成任务为 `P9-T01`，最新提交 `P9-T01-a` 是其已完成前置 baseline 修复。下一步读取 `TODO-7.md` 中 `P9-T01` 的详细要求。
- 已读取 `TODO-7.md` 的 `P9-T01` 任务体。当前执行步骤：定位 `InstanceKey` / `TemplateKey` / `ExternAbi` / `devirtualize` 的定义与引用，随后做最小迁移和删除。
- 已完成第一轮代码编辑：`InstanceKey` / `TemplateKey` 下沉到 `scoopc_ids`，`ExternAbi` 下沉到 `scoopc_types`，root-level `devirtualize.rs` 的 MIR 仍用逻辑迁入 `mir::dispatch_devirtualize` 后删除。下一步运行格式化和编译以发现迁移遗漏。
- `cargo fmt`、`cargo build --workspace` 和 `cargo run -p scoop_tools -- dependency-gate` 已通过；指定反向边搜索中 `hir/typecheck` 的 MIR/HIR import 已归零，LLVM 仅剩测试命中，`effect_lowered` 仅剩 P9-T01-a 已记录的 LIR-owned HIR payload namespace。下一步运行全量测试和 clippy。
- `cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings` 与 `git diff --check` 已通过。已将 `P9-T01` 在 `TODO.md` 和 `TODO-7.md` 标记为 `[DONE]` 并补充完成记录；下一步检查最终差异并提交。
