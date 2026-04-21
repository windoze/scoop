# 执行计划

## 说明

按要求先记录计划。这里记录的是可审计的推理摘要、假设、执行步骤与进度更新，不写入冗长的内部隐式思维内容。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或首个子任务。
5. 运行相关测试、格式化、lint，修复发现的问题。
6. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录进展。
7. 提交 Git commit。
8. 停止，不继续处理后续任务。

## 当前已知约束

- 必须先处理最新提交中提到的既有问题，再进入 `TODO.md` 任务。
- 如遇到规范不匹配、缺失语言特性或需要依赖前置修复，不允许绕过，必须先把前置问题写入 `TODO.md`/`PLAN.md` 并调整顺序。
- 需要保证编译、测试和 lint 无警告，至少包括 `cargo clippy --all-targets -- -D warnings`。

## 待更新项

- 最新提交中是否包含待修复问题。
- `TODO.md` 中首个未完成任务的内容。
- 是否需要任务拆分。
- 实施结果、测试结果、提交信息。

## 最新进展

- 已检查最新提交 `dd188d33e9f243e3940a3adf9533e98651a1b9ef`，提交信息为 `[T4016b4b0] Restore cross-thread GC stress regression fixture`。
- 该提交信息和摘要未额外声明新的“必须先修复的既有问题”；当前仍按 `TODO.md` 顺序推进。
- 已读取 `TODO.md` / `PLAN.md`：
  - 顶层 `T4016`、`T4016b`、`T4016b4` 仍为父级拆分任务，不作为本轮直接执行对象。
  - 首个未完成的叶子任务是 `T4016b4b`：完成 pure `Continuation<Resume>` shorthand 的收尾迁移，并做全量 `run-pass` 验收。

## 当前执行策略

1. 先全量运行 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`，确认 `T4016b4b` 当前的真实失败点。
2. 并行搜索仓库内剩余 `Continuation<Resume>` / 相关 legacy shorthand 用法，定位可能未迁移的 fixture、测试或源码。
3. 若全量回归失败是 pure shorthand 残留导致：
   - 直接修复源码或 fixture，补必要回归；
   - 重新跑定向测试，再回到全量 `run-pass`。
4. 若失败暴露出新的前置实现缺口且无法在本任务内直接闭环：
   - 按要求把该前置缺口写入 `TODO.md` / `PLAN.md`，重排依赖顺序；
   - 提交并停止。
5. 若 `T4016b4b` 验收通过：
   - 更新 `TODO.md` / `PLAN.md` / 本文件；
   - 运行必要的 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`；
   - 提交并停止。

## 本轮结果

- `T4016b4b` 已完成。
- 盘点结果：
  - 仓库内剩余 `Continuation<Resume>` 文本匹配主要位于文档、任务记录、diagnostic 文案与 removed-shorthand typecheck fixture。
  - 未再发现会进入生产/codegen 主线的 legacy pure shorthand。
- 已完成验收：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` -> `fixtures: ok (375)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
- 文档状态需要同步为：
  - `TODO.md` 中将 `T4016b4b` 标记为完成；
  - 连带将其父任务 `T4016b4`、`T4016b` 标记为完成；
  - `PLAN.md` 的下一步切换到 `T4016d`。
