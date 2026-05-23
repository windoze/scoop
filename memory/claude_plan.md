# 执行计划

## 当前状态

- 已收到要求：按 `TODO.md` 的顺序只完成第一个未完成任务，然后提交并停止。
- 本文件记录可审查的执行计划、关键决策和进度更新；不记录内部隐含推理。

## 初始计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只处理会阻塞当前任务的问题。
3. 阅读当前任务涉及的代码、测试、文档和约束，避免做开放式历史问题扫查。
4. 如任务可直接完成，做最小且完整的实现，并补充或调整相关测试/fixture。
5. 运行任务要求的验证命令和必要的相关测试；若发现未排期失败，修复或在 `TODO.md` 中添加最小前置任务并停止。
6. 任务完成后，将 `TODO.md` 中该任务标题加 `[DONE]`，更新完成记录；仅在阶段计划实际变化时修改 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关变更，然后停止，不继续下一个任务。

## 进度日志

- 已完成：读取 `TODO.md`，第一个未完成任务为 `P9-T06-b`（发布 LIR-owned ordinary-callee suspend 合同）。
- 已检查：最近提交 `e6365bbe [P9-T06] Schedule ordinary-callee suspend prerequisite` 与当前任务直接相关，按要求纳入当前任务上下文。
- 已读取：`TODO-7.md` 中 `P9-T06-b` 要求发布 LIR-owned ordinary-callee suspend 合同，消除 LLVM production 对 `crate::effect` / future effect stage 的直接依赖。
- 已执行结构迁移：ordinary-callee suspend planning 代码从 `effect` owner 移到 `effect_lowered::ordinary_callee`，并开始改造生产路径 import 与 HIR/AST 直接引用。
- 已完成实现：LLVM production 与 `llvm_codegen_stage` 不再引用 `crate::effect`；dependency gate 增加 effect-stage residual 防回归规则；`P9-T06` 后续范围已同步为剩余 effect-facts builder/stage glue + LIR crate 抽取。
- 已完成验证：`cargo fmt`、`cargo build --workspace`、`cargo test --all --all-targets`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、指定 residual 搜索、`git diff --check` 均通过。
- 下一步：检查最终 diff/status，提交本次任务变更。
