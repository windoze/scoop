# 当前执行计划

说明：我不能记录完整的私人推理链；本文件记录可审计的执行计划、关键依据、进度和验证步骤。

## 目标

- 按 `TODO.md` 的顺序识别第一个标题未带 `[DONE]` 的任务。
- 只完成该一个任务；完成后更新 `TODO.md`、运行相关验证、提交 Git，然后停止。

## 步骤

1. 阅读 `TODO.md`，确认第一个未完成任务及其依赖、验收标准和完成记录要求。
2. 查看最近提交信息，判断是否明确提到与该任务直接相关的未完成问题。
3. 按任务要求检查相关源码、测试、fixture 和文档，避免开放式历史问题扫描。
4. 如果发现阻塞当前任务的真实前置缺口，更新 `TODO.md` 添加最小必要前置任务并提交后停止。
5. 如果可直接执行，实施最小正确代码/测试/文档变更。
6. 运行该任务要求的验证命令；若失败，修复同一根因下的相关问题并重跑验证。
7. 将任务标题加 `[DONE]`，更新完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查 `git status`、`git diff`、最近提交，提交本次任务涉及的全部未提交变更。
9. 停止，不处理下一个任务。

## 进度

- 已创建本计划文件，下一步读取 `TODO.md` 并定位第一个未完成任务。
- 已确认第一个未完成任务为 `P6-T02`：实现 prelude package 列表并与 auto dependency 解耦。
- 最近提交为 `[P6-T01] Implement sysroot auto dependencies`，未在提交标题中暴露新的未完成 blocker；后续只检查与 `P6-T02` 直接相关的 resolver/import 与 sysroot dependency 配置。
- 代码检查发现 resolver 已硬编码 `scoop.core` / `scoop.lang.string` 自动 star import，但缺少 prelude package 未加载时的稳定 compiler-configuration diagnostic；auto dependency 中的 `scoop.collections` / `scoop.delegates` 也需要明确 fixture 证明不会自动短名可见。
- 执行方案更新：在 `resolve::imports` 中引入明确 prelude package 列表与校验错误，更新 direct type-resolution 使用同一列表，补充单元测试和 typecheck fixtures 后运行定向及全量验证。
- 已完成实现：prelude list 固定为 `scoop.core` / `scoop.lang.string`；完整编译配置下缺失 prelude cone 会报 `scoop::resolve::prelude_package_not_loaded`；新增 fixtures 覆盖 prelude 正向、`scoop.collections` 非 prelude 短名负向和显式 import 正向。
- 已完成验证：`cargo fmt`、`cargo test -p scoopc prelude -- --nocapture`、新增 fixtures 定向验证、`cargo run -p scoop -- test tests/fixtures/typecheck/`、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo run -p scoop -- test` 均通过。
- 已更新 `TODO.md`，将 `P6-T02` 标记为 `[DONE]` 并写入完成记录；下一步检查 git diff/status 后提交本任务变更。
