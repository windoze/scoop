# 当前执行计划

说明：本文件记录可审计的执行计划、约束、关键决策和进度更新；不记录私有推理链。

## 初始约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一权威来源。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若遇到阻塞当前任务的缺陷、规格不匹配或缺失特性，优先修复；若无法在当前任务中正确修复，则在 `TODO.md` 中插入最小必要前置任务并停止。
- 不通过缩小范围、改 fixture 形状、特殊分支或其他 workaround 绕过问题。
- 任务完成后必须更新 `TODO.md` 标题为 `[DONE]`，补全完成记录，运行相关验证，并提交 Git commit。
- 仅当阶段级计划、依赖或完成标准发生变化时更新 `PLAN.md`。

## 步骤计划

1. 阅读 `TODO.md`，确定第一个未完成任务及其验收要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只处理与当前任务直接相关的内容。
3. 根据任务内容读取最小必要代码、测试和文档上下文。
4. 实现当前任务，优先做最小但完整、规格正确的改动。
5. 添加或更新最相关的测试/fixture，避免 fixture-only hack。
6. 运行任务要求的验证；若发现未排期的失败测试/fixture，修复或在 `TODO.md` 中安排为当前任务前置项。
7. 更新 `TODO.md`：将当前任务标题加 `[DONE]` 并填写完成记录；如发现必须拆分或新增前置任务，则按依赖顺序更新并停止。
8. 视情况更新 `PLAN.md`，仅记录阶段级变化。
9. 检查 `git status`、`git diff`、最近提交，确认只提交本轮相关改动。
10. 创建描述清晰的 Git commit。
11. 停止，不继续下一个任务。

## 当前状态

- 状态：已确认第一个未完成任务为 `P9-T02R：Review scoopc_ast 抽取`。
- 最近提交：`44c0c62d [P9-T02] Extract scoopc_ast crate`，与当前 review 任务直接相关；本轮只复审该抽取结果。
- 已完成复审项：`crates/scoopc/src/{ast,parser,syntax}` 实体目录不存在；`crates/scoopc/src/lib.rs` 保留 `scoopc::ast`、`scoopc::parser`、`scoopc::syntax` façade；`cargo tree -p scoopc_ast` 显示 workspace 依赖仅为 `scoopc_source`、`scoopc_span`、`scoopc_types`；`dependency-gate` 已通过并覆盖 `scoopc_ast` base-only stage。
- 验证结果：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、`cargo tree -p scoopc_ast`、`git diff --check` 均已通过。
- TODO 更新：`TODO.md` 与 `TODO-7.md` 已将 `P9-T02R` 标记为 `[DONE]`，并记录 review 结论；下一步检查 diff/status 后提交。
