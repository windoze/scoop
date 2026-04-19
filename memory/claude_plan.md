# 本轮执行计划（初始版）

## 说明

按要求先落盘执行计划与决策摘要。这里记录的是可审计的计划、假设、检查点和后续变更，不包含逐字内部思维。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现其前置依赖缺失、规格不匹配或任务过大，则先按要求重排/拆分任务并更新 `TODO.md`、`PLAN.md`，随后停止或执行拆分后的首个子任务。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交说明中是否提到已有问题；若提到，先定位并修复这些既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 结合 `PLAN.md`、相关代码与测试，评估该任务是否可在本轮完整落地。
4. 如果任务过大或存在缺失前置条件：
   - 在 `PLAN.md` 中细化为更小的子任务；
   - 在 `TODO.md` 中重排并把首个子任务放到当前执行位置；
   - 仅执行当前应完成的第一个任务，然后停止。
5. 实现任务，补充/调整测试，并运行必要验证：
   - 至少运行与改动直接相关的测试；
   - 若改动范围允许，运行更完整的测试与质量检查；
   - 目标是无编译告警，并尽量满足 `cargo clippy --all-targets -- -D warnings`。
6. 更新文档与计划：
   - 在 `TODO.md` 标记当前任务完成；
   - 在 `PLAN.md` 记录状态变化、依赖变化和后续安排；
   - 按要求持续更新本文件，记录关键进展和计划调整。
7. 使用清晰的 Git 提交信息提交本轮更改，然后停止，不进入下一个任务。

## 关键约束

- 不接受 workaround、shim、仅夹具生效的补丁或偏离规格的“暂时方案”。
- 一旦发现规格缺口、语言特性缺失、实现边界不完整或运行时/编译器 bug，必须先在 `TODO.md` 中显式建模为前置任务，并调整顺序。
- 不回退或覆盖与本轮任务无关的现有修改。
- 如发现 `PROMPT.md` 被意外修改，需一并纳入提交。

## 待确认信息

- 最新提交是否显式提到待修复问题。
- `TODO.md` 中当前第一个未完成任务是什么。
- 该任务涉及的模块、测试范围、潜在依赖和规格依据。

## 进展记录

- 已创建本文件并写入初始计划，下一步开始检查最新提交与任务列表。
- 已检查最新提交 `6f46d454f001309fd1a7999bd3c188d294c7eb5c`（`[T4009b] Define task poll and step contract`）：
  - 提交说明与变更摘要没有额外点名“必须先修的既有 issue”；
  - 当前可继续按 `TODO.md` 顺序推进下一项未完成任务。
- 已定位 `TODO.md` 中首个未完成任务为 `T4009c`：将 `spawn` / `join` 从当前 `Task` core 语义中移出，并明确留待后续 structured concurrency。
- 初步范围评估：
  - 当前仓库仍在 parser / AST / typecheck / HIR lowering / fixtures / spec 文案中保留 `spawn` / `join` 的“早期最小模型”痕迹；
  - `Task<T>` 核心主线本轮应收敛到 `async {}` / `async fun` + `await` + `Task.poll()/step()` + 内部 helper；
  - 预期做法是把 `spawn` / `join` 改成显式 deferred/unsupported 的语言表面，并同步更新文档、夹具与验证集，而不是继续保留“临时可执行语义”。
- 当前判断：`T4009c` 规模可控，不需要进一步拆分；下一步直接实施代码与文档收口。
- 已完成 `T4009c` 主体实现：
  - 公开 `spawn` / `join` surface 已从当前 `Task` core 语义中移出；
  - parser / AST 保留语法壳，但 typecheck 统一报 `structured_concurrency_deferred`；
  - HIR lowering 不再为 `spawn` / `join` 保留“最小可执行语义”，只留下 deferred 占位；
  - spec / runtime / sysroot 文档已同步收口为“当前只有 `async {}` / `async fun` / `await` / `Task.poll()/step()` 是 `Task` core，structured concurrency 留待后续”。
- 实施过程中发现并修复了一个既有 bug：
  - 当 `async` 体的尾值直接是 `await task` 且结果为 GC 引用类型（例如 `String`）时，结果会在进入 `step_ready(...)` 前丢失；
  - 已在 `crates/scoopc/src/hir/lower/block.rs` 中改为先物化 async body 尾值，再交给 `__task_ready_value` / `step_ready(...)`，并用新的 `async_await_string_basic` 回归覆盖。
- 已完成验证：
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -q -p scoop -- test`
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步只剩按要求更新 Git 状态、提交本轮变更并停止；下次调用应从 `T4009h` 开始。
