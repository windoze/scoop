## 执行记录

说明：此文件记录可公开的执行计划、关键判断、进度与变更；不包含不可公开的内部推理细节。

### 初始计划

1. 查看最新提交，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务。
3. 如果该任务过大，先拆分任务，并同步更新 `TODO.md` / `PLAN.md`。
4. 实现当前应执行的首个任务。
5. 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md` 与本文件中的进度记录。
7. 按仓库约定创建一次提交，然后停止，不继续处理下一个任务。

### 当前状态

- 已创建初始执行记录。
- 已检查最新提交：`[T5001fR] Review default explicit root-frame route`，提交标题未声明需要优先处理的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`。
- 当前首个未完成任务：`T5001g 全量回归、GC stress、verify-roots 与文档收尾`。

### T5001g 执行步骤

1. 运行任务要求的最小验收矩阵：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
2. 运行质量门禁：
   - `cargo clippy --all-targets -- -D warnings`
3. 若回归或门禁失败：先定位并修复失败，再重新验证。
4. 复核并补齐文档：重点检查 `SCOOP_RUNTIME.md`、必要实现注释，以及默认 explicit mode 的行为说明是否与现状一致。
5. 记录对象体积 / 二进制体积与 steady-state / GC pause 的观察；若只能得到有限观测，则如实记录，不把性能调优作为 blocker。
6. 更新 `TODO.md`、`PLAN.md` 与本文件后提交一次变更，并停止。

### 进度更新

- `cargo test --all` 已通过。
- `cargo run -p scoop -- test` 失败，首个失败 fixture：`tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`。
- 该失败属于执行过程中发现的现存问题，已转为当前优先处理项；下一步先最小复现并定位根因。

### blocker 结论

- 已用更小复现确认：
  - 简单 `async { 41 }` + 单次 `task.step()` 正常；
  - 一旦进入 `await` / waiting path，外层 task 首次 `step()` 返回 `Pending` 后，后续 drive 会卡住。
- 影响范围至少包括：
  - `tests/fixtures/run-pass/task_step_manual_basic.scoop`
  - `tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`
  - `tests/fixtures/run-pass/async_await_string_basic.scoop`
  - `tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
- 这说明当前并不是 `T5001g` 验收矩阵本身的问题，而是 async/task waiting transport 主线存在真实 regression，会阻断全量回归结论。
- 处理决定：按照依赖顺序，把该问题显式插入 `TODO.md` / `PLAN.md` 作为 `T5001f1/T5001f1R` 前置任务；本次提交只记录 blocker 与重排，不继续越过它做 `T5001g`。
