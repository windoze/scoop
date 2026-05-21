# 当前执行计划

## 目标

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未以 `[DONE]` 标记的任务，然后停止。
- 若任务被具体前置问题阻塞，只添加最小必要前置任务并提交，不继续推进被阻塞任务。
- 不采用规避实现、夹具专用逻辑或弱化规格的做法。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 查看最近提交，判断是否明确提到与该任务直接相关的未完成问题。
3. 阅读当前任务要求、依赖和验证要求；只在必要范围内查看相关代码、测试和文档。
4. 若发现当前任务依赖的具体缺失功能或阻塞缺陷，更新 `TODO.md` 插入最小前置任务，必要时更新 `PLAN.md`，提交后停止。
5. 否则实现当前任务，保持改动最小且符合现有代码结构。
6. 运行任务要求的验证，并补充运行相关测试；若失败，修复后重跑。
7. 在 `TODO.md` 中给当前任务标题添加 `[DONE]` 并更新完成记录；只在阶段计划实际变化时更新 `PLAN.md`。
8. 检查工作区差异，确认包含本次任务所需变更。
9. 按仓库风格提交所有相关变更。
10. 停止，不处理下一个任务。

## 进度

- 已创建执行计划文件。
- 已读取 `TODO.md` 并定位第一个未完成任务：`P3-T06`，目标是迁移 dispatch 去虚化到 MIR pass 并删除 HIR owner。
- 已读取 `TODO-4.md` 中的 `P3-T06` 详情。
- 最近提交 `P3-T05R` 明确说明 dispatch 去虚化仍待 `P3-T06` 迁移，属于当前任务范围；未发现需要先插入的新前置任务。
- 已定位现有实现：HIR lowering 通过 `devirtualize_dispatch_calls` 调用 `try_devirtualize_dispatch_target`，MIR materialization rewrite 也会在 substitution 中直接把 virtual/interface call 改成 direct call；显式 MIR pass pipeline 目前尚未调度 `MirPassKind::Devirtualization`。
- 当前实现计划：新增 MIR dispatch devirtualization pass 并加入 pipeline；materialization 不再改写 dispatch call；HIR lowering 一律保留 dynamic dispatch 并只发布 dispatch call-site contract；更新相关测试与 fixture。
- 已实现核心迁移：`try_devirtualize_dispatch_target` 的活跃非后端调用点移至 MIR devirtualization pass；HIR `devirtualize_dispatch_calls` 已删除；materialization rewrite 不再改写 dispatch call，只做 substitution 和 dispatch candidate instance discovery，并把 canonical target 作为 pass facts 记录。
- 已修复迁移后暴露的问题：effect-generic dispatch candidate 需要在 materialization substitution 阶段发现实例；pass-published rewritten body 需要规整重复 `SiteId`。
- 已通过 P3-T06 指定验证：`cargo fmt`、`cargo test -p scoopc --no-default-features hir --lib`、`cargo test -p scoopc --no-default-features mir::materialize --lib`、`cargo test -p scoopc --no-default-features monomorph --lib`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_materialized`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 搜索 `devirtualize_dispatch_calls` 无命中；`try_devirtualize_dispatch_target(` 的非测试活跃调用点只剩 MIR pass 以及 P7 归属的 LLVM codegen/reachability 残留。
- 已更新 `TODO.md` 与 `TODO-4.md`，标记 `P3-T06` 完成并填写完成记录。
- 下一步提交本任务改动并停止。
