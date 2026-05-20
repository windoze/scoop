# 当前执行计划

## 原则

- 以 `TODO.md` 为唯一任务顺序来源，先识别第一个标题未带 `[DONE]` 的任务。
- 本轮只完成第一个未完成任务；完成后更新记录、提交 Git，并停止。
- 如果遇到阻塞当前任务的规范不匹配、缺失功能或实现边界，不做绕路；在 `TODO.md` 中添加最小必要前置任务并提交后停止。
- 不把内部推理过程写入日志；本文件记录可审查的计划、依据、进度和验证结果。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务及其验证要求。
2. 查看最近提交信息，判断是否有明确提到且直接关联该任务的未完成事项。
3. 按任务要求阅读相关代码、规格和测试，确认实现边界。
4. 进行最小正确实现；如需修改计划或完成关键步骤，同步更新本文件。
5. 运行任务要求的验证，以及必要的相关测试和格式/编译检查。
6. 更新 `TODO.md`：给完成的任务标题加 `[DONE]`，并补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff`、最近提交记录，确认只提交本轮相关变更。
8. 使用清晰任务编号提交，提交后停止，不处理下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P4-T01：重排 sysroot 到 sysroot/lib/<cone>/src`。
- 最近提交是 `[P3-T03] Enforce trusted syslib privileges`，未在提交标题中暴露与 `P4-T01` 直接相关的未完成事项。
- 已将现有 sysroot `.scoop` 源码移动到 `sysroot/lib/<cone>/src/`，并为每个现有 sysroot cone 添加 `Cone.toml`。
- kind 分类：`scoop.core`、`scoop.unsafe`、`scoop.collections`、`scoop.delegates`、`scoop.thread`、`scoop.sync`、`scoop.runtime.test` 先按保守策略设为 `syslib`；`scoop.lang.string` 当前只使用普通 Scoop 与 `@Extern`，设为 `lib`。
- 为避免现有 overlay fixture 在 base path 变化后不再替换 `scoop.core/src/core.scoop`，已同步把 active `.sysroot/scoop.core/*.scoop` overlay 移到 `.sysroot/lib/scoop.core/src/*.scoop`。
- 验证进展：`cargo build`、`cargo test -p scoopc sysroot -- --nocapture`、`cargo run -p scoop -- test tests/fixtures/build/`、`cargo run -p scoop -- test tests/fixtures/typecheck/`、`cargo clippy --all-targets -- -D warnings` 已通过。
- 完整 fixture 首次运行发现 5 个 HIR golden 仅因 `sysroot/lib/scoop.core/src/print.scoop` 注释路径更新导致 core declaration span 后移 8 字节失败；已同步对应 `.hir` golden，下一步重跑 HIR 与完整 fixture suite。
- HIR fixture 分组重跑通过；完整 fixture suite 重跑通过（1563 checks）；`cargo test --all --all-targets` 通过。
- `TODO.md` 已将 `P4-T01` 标记为 `[DONE]`，补全完成记录，并把后续 P8 任务中的 sysroot 入口路径更新为 `sysroot/lib/<cone>/src/...` / `native/`。
- 下一步检查 git diff/status 并提交本轮变更。
