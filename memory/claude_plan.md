# 执行计划

## 约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一依据。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后停止。
- 如果遇到阻塞当前任务的未排期缺陷或缺失特性，先在 `TODO.md` 中加入最小必要前置任务并提交，然后停止。
- 不使用规避实现，不降低测试或 fixture 覆盖要求。
- 完成任务后更新 `TODO.md` 的标题和完成记录，并按要求提交。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务及其依赖、验证要求和完成记录格式。
2. 查看最近提交，确认是否有与该任务直接相关的未完成事项。
3. 按任务要求检查相关代码、fixture、测试和文档，只围绕当前任务建立必要上下文。
4. 实现当前任务，优先采用最小正确变更，并避免修改无关文件。
5. 运行任务要求的验证命令；若发现未排期失败，修复或在 `TODO.md` 中加入必要前置任务。
6. 更新 `TODO.md`，将当前任务标题标记为 `[DONE]` 并补充完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff` 和最近提交，提交本次任务相关改动。
8. 停止，不继续处理下一个任务。

## 当前进度

- 已创建初始执行计划。
- 已读取 `TODO.md`，确认首个未完成任务是 `P9-T04R：Review scoopc_hir 抽取`。
- 最新提交为 `[P9-T04] Extract scoopc_hir crate`，与当前 review 直接相关；当前未跟踪的 `PLUGIN_ABI.md` 不是本任务触碰面，除非后续发现其与当前任务直接相关，否则不修改、不提交。
- 下一步围绕 `P9-T04R` 检查 `scoopc_hir` crate 依赖、façade、dependency gate、vtable/itable 归属和残余 cross-stage 引用。
- 已完成静态 review：`scoopc_hir` 的 Cargo 依赖只包含 base crates、`scoopc_ast`、`scoopc_hir_facts` 与外部诊断/日志依赖；`scoopc_hir/src` 未发现对 `scoopc_mir`、`scoopc_codegen_llvm`、`scoopc::mir` 或 `crate::mir` 的引用；`vtable.rs` / `itable.rs` 只存在于 `crates/scoopc_hir/src/`，后端仅消费前端发布的表数据。
- 下一步运行 `P9-T04` 全套验证命令；若出现未排期失败，将先修复或回写 TODO 前置任务。
- 验证已通过：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、`cargo tree -p scoopc_hir`、`git diff --check`。
- 已更新 `TODO.md` 与 `TODO-7.md`，将 `P9-T04R` 标记为 `[DONE]` 并记录 review 结论；下一步检查 diff 和工作树后提交。
- 提交前检查已完成：本次计划提交 `TODO.md`、`TODO-7.md` 与 `memory/claude_plan.md`；未跟踪的 `PLUGIN_ABI.md` 与当前任务无关，保持不触碰。
