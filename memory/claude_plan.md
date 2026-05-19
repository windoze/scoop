# 执行计划

本文件记录本轮调用的可公开执行计划与进度。不会包含私有推理链；只记录可审查的操作步骤、决策依据和关键结果。

## 当前计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判定第一个未完成任务。
2. 读取该任务相关上下文；仅在必要时查看 `PLAN.md`、最近提交和相关代码/测试。
3. 如该任务可直接完成，实施最小正确改动，并避免规避语言/运行时/规格缺口。
4. 运行任务要求的验证命令和必要的回归测试；若发现阻塞当前任务的真实缺口，先修复，或在 `TODO.md` 中插入最小前置任务后停止。
5. 更新 `TODO.md`，在完成任务标题前加 `[DONE]` 并填写完成记录；只有阶段级计划变化时才更新 `PLAN.md`。
6. 检查工作区差异，提交本轮所有相关改动，然后停止，不继续下一个任务。

## 进度

- 已创建本轮执行计划，下一步读取 `TODO.md` 确认第一个未完成任务。
- 已确认第一个未完成任务为 `P2-T01`：`@Extern` 支持 `callingConvention` property。
- 下一步检查最近提交是否有直接相关未完成事项，并阅读注解解析/typecheck/HIR/codegen中现有 calling convention 路径。
- 最近提交为 `P1-T03` archive fixture 退场，未发现直接相关未完成事项。
- 已发现 `ExternFun.calling_convention` 目前从叠加的 `@CallingConvention` 读取；本轮改为从 `@Extern(..., callingConvention = "...")` 读取，并新增旧叠加写法的稳定拒绝。
- 已完成第一轮实现编辑：typecheck 接受/校验 `callingConvention` 属性，`abi = "scoop"` 属性形态拒绝，旧 `@Extern + @CallingConvention` 叠加写法拒绝，HIR side table 从 `@Extern` 属性保存 calling convention；已新增 parse/typecheck fixtures 和 HIR 单测。
- 已通过：`cargo fmt`、`cargo test -p scoopc hir_collects_extern_calling_convention_property -- --nocapture`、`cargo test -p scoopc typecheck::annotations -- --nocapture`（0 个匹配单测）、新增 parse fixture 定向运行、`tests/fixtures/typecheck/` 目录运行（496 checks）。
- `cargo test --all --all-targets` 首次发现 UMB fixture 必须保留 `EXPECT-ERROR-AT`，已补回新诊断位置并通过 `cargo test -p scoopc audit::spec_coverage::umb_fix_fixture_index_in_sync -- --nocapture`。
- 已通过全量验证：`cargo build`、`cargo clippy --all-targets -- -D warnings`、重跑 `cargo test --all --all-targets`（905 passed）、`cargo run -p scoop -- test`（1556 checks）。
- 已更新 `TODO.md`：任务索引与标题均将 `P2-T01` 标为 `[DONE]`，当前状态推进到 `P2-T02`，并补充完成记录；`PLAN.md` 无阶段级变更。
- 提交前检查确认工作区只包含本轮 `P2-T01` 相关代码、fixtures、`TODO.md` 与本计划文件改动；下一步提交。
