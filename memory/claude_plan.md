# 执行计划

说明：出于安全边界，这里记录可审计的执行计划、假设、决策与进度，不记录不可审计的完整内部推理细节。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现前置缺陷或规范不匹配，会先修复或把阻塞项前移，然后停止。

## 初始步骤

1. 检查最新一次 Git 提交的信息，确认是否提到已知问题、遗留缺陷或需要先处理的事项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 如该任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
4. 在实现前阅读相关代码、测试、规范与文档，确认实现边界和依赖。

## 执行原则

- 不接受绕过实现、临时兼容层、只改夹具的“伪完成”方案。
- 一旦遇到规范不匹配、语言特性缺失、实现边界不完整或真实缺陷，必须先把该问题写入 `TODO.md`，调整依赖顺序，并更新 `PLAN.md` 说明阻塞原因。
- 只完成一个任务后停止。

## 实施与验证

1. 实现目标任务。
2. 运行相关测试。
3. 运行必要的质量检查，至少覆盖本次改动范围；如果可行，运行 `cargo clippy --all-targets -- -D warnings`。
4. 修复测试或检查中暴露的问题，直到结果稳定。

## 文档与提交流程

1. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
2. 更新 `TODO.md`，将本轮完成的任务标记为已完成；若阻塞，则按依赖顺序重排并保持为待办。
3. 更新 `PLAN.md` 反映当前状态、拆分结果或阻塞说明。
4. 提交 Git，提交信息应清晰描述本轮任务。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交：`462222c5ba35580ae9aee33837f237ba1b2b653a`，提交信息只有 `[T4009a1] Unify async surface as Task sugar`，未额外记录需要优先修复的遗留 issue。
- 已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`，当前首个未完成任务为 `T4009a2`：从 core / runtime / sysroot / stdlib 移除当前 executor-centric `Task` surface。
- 已完成公开 surface 清理：删除 `sysroot/task.scoop` / `stdlib/task.scoop` 与相关 executor/task adapter fixtures；`async` caller 的同步驱动回归改为 handled `Async.await(...) -> __scoop_task_join(...)`。
- 已完成 runtime 收口：`runtime/c/scoop_task_executor.c` 已替换为 executor-free 的 `runtime/c/scoop_task.c`，只保留 lazy create / completed-from-result / synchronous join 三条最小 `Task` core 路径。
- 已顺手修复既有 GC 裂缝：RTTI trace bitmap 不再把 `Task` 误判为 non-GC handle，并新增 `gc_trace_task_field_basic` 回归。
- 已完成验证：
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`：`fixtures: ok (334)`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`：`fixtures: ok (357)`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_join -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join`
  - `cargo run -q -p scoop -- test`：`fixtures: ok (1070)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：更新 `TODO.md` / `PLAN.md` 状态后提交 Git，并停止在 `T4009a2`。

## 当前判断

- `T4009a2` 虽然影响面较广，但切面明确，可在一轮内完整完成，暂不继续拆分子任务。
- 目标不是先定义最终 `poll()` API；本轮只移除错误方向的公开 surface，并保留最小 `Task` core 可执行路径。

## 本轮实施方案

1. 删除公开的 `scoop.task` sysroot / stdlib surface，不再对外暴露 `Executor`、`taskCreate`、`tryStart`、`onComplete`、`map/andThen/await` 等 executor-centric API。
2. 将 runtime 中与 `Executor` 强绑定的实现从 `scoop_task_executor.c` 中拆除，保留最小 `Task` core runtime：
   - `Task` 对象分配；
   - `async` sugar 需要的内部 create 路径；
   - `spawn` 现阶段使用的“已完成 task”包装路径；
   - `join` 的同步直驱读取路径。
3. 更新编译器内部 lowering 所使用的 task-create helper FQN，使其不再依赖已删除的 `scoop.task.taskCreate` 公开入口。
4. 清理/改写依赖 executor public surface 的 fixtures 与 runtime tests：
   - 保留 `Task` core、`async`、`spawn`、`join` 的回归；
   - 移除仅验证旧 `Executor` / stdlib task adapters 的夹具与测试。
5. 同步更新 `SCOOP_FULL_SPEC.md` / `sysroot/core.scoop` 等文档注释，明确当前阶段不提供公开 executor surface。
6. 运行定向测试，再运行 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 风险点

- 删除 `scoop.task` 后，现有 run-pass/typecheck 夹具需要改为直接使用 `scoop.core.Task` 与 `join` 驱动，否则会整体失效。
- runtime 文件拆分后，`build.rs`、ABI allowlist、runtime 单测需要同步更新，否则会出现链接或导出清单不一致。
