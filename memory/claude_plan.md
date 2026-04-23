# 当前执行计划（公开版决策摘要）

说明：我不会写入逐字逐句的内部推理，但会在这里持续维护足够详细的执行计划、判断依据摘要、关键发现与进展，便于随时审阅。

## 初始目标

本轮只完成一件事：找到 `TODO.md` 中第一个未完成任务，将其完整实现、测试、更新文档与任务状态，并提交一个 Git commit 后停止。

## 初始执行步骤

1. 检查最新一条 Git commit 的说明，确认其中是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前整体计划与该任务上下文。
4. 结合代码现状评估该任务是否可以在本轮完整完成：
   - 如果可以，直接实施。
   - 如果任务过大，则先把任务拆分为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
5. 在实施前检查相关代码、测试、规范或夹具，确认不存在阻塞该任务的既有缺陷。
6. 如果发现任何先于当前任务的既有问题、规格不匹配、实现边界缺失或测试回归：
   - 先修复该问题；如果本轮无法直接修复，则把它作为前置任务插入 `TODO.md` 当前任务之前，更新 `PLAN.md`，提交后停止。
7. 完成代码修改后，运行相关格式化、测试和必要的 lint / clippy，直到没有相关警告与失败。
8. 更新 `TODO.md` 与 `PLAN.md`，记录本轮完成情况和后续状态。
9. 提交 Git commit，提交信息将与任务编号/内容对应。
10. 停止，不继续下一个任务。

## 当前已知约束

- 必须优先处理最新提交中提到的遗留问题。
- 不允许通过变通、夹具特判、缩小语义范围等方式绕过规格缺口。
- 如果实现过程中发现新的前置缺陷，必须先修复或先把它登记为更早的任务。
- 需要尽量保证 `cargo clippy --all-targets -- -D warnings` 通过。
- 需要保持 `README.md`、`PLAN.md`、`TODO.md` 与实际状态一致。

## 当前结论（已探查）

1. 最新提交为 `[T4016T6] Replace task mutex with atomic claim field`，提交说明本身没有额外声明一个必须优先插队修复的遗留问题。
2. `TODO.md` 中第一个未完成任务是 `T4016T7`：用轻量 claim bit 重写 `Task.step()`，并把并发 / reentrant 误用收口为 trap。
3. 目前未发现必须先于 `T4016T7` 新增到 `TODO.md` 更前位置的 blocker，因此本轮直接实现 `T4016T7`。

## T4016T7 的具体实现计划

1. 修改 `sysroot/task.scoop`：
   - 去掉当前 `__task_claim_acquire()` 中“claim 失败就 `yield()` 重试”的行为。
   - 改成一次性 try-claim；若 claim 失败则直接 trap（当前阶段沿用 `exit(3)` 作为 fatal trap 表达）。
   - 把 `Task.step()` 中 `Running -> Pending()` 的过渡逻辑改成 trap。
   - 保持“运行用户代码 / `Continuation.resume(...)` 时不持有 claim；返回后重新 claim 发布状态”的主线不变。
2. 视需要补最小内部 helper（例如统一的 trap helper），但不引入新的 runtime ABI 或 task 特判路径。
3. 补三类定向回归：
   - 单线程手动 drive 现有行为保持正常。
   - 跨线程顺序 handoff：前一个 `step()` 返回后，另一个线程可以继续 drive 同一 task。
   - 误用 trap：覆盖至少一个 reentrant `step()`，以及至少一个并发 `step()` 竞争场景，稳定以 `EXPECT-EXIT: 3` 结束。
4. 运行定向测试；若通过，再跑更大范围的 `cargo run -p scoop -- test`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 完成后更新 `TODO.md` / `PLAN.md`，把 `T4016T7` 标记为完成并记录验证结果，然后提交 commit 并停止。

## 当前风险点

- `Task.step()` 在释放 claim 后执行用户代码，这意味着“并发误用”通常会表现为：另一个 driver 成功 claim，但观察到 `Running` 后 trap；实现时要确保这一点是稳定且可回归的。
- reentrant trap 需要用最小、确定性的 fixture 表达，避免依赖不稳定时序。
- cross-thread sequential handoff 需要与“并发误用 trap”区分开：只允许前一轮 drive 已经发布新状态并返回之后再换线程继续 drive。

## 本轮完成情况

1. 已完成 `T4016T7`，没有发现必须前插到 `TODO.md` 更前位置的新 blocker。
2. 已实施的代码改动：
   - `sysroot/task.scoop`：`__task_claim_acquire()` 改成单次 try-claim；失败直接 `exit(3)`。
   - `sysroot/task.scoop`：`Task.step()` 中 `Running -> Pending()` 过渡逻辑删除，改成直接 trap。
   - `sysroot/core.scoop` / `sysroot/task.scoop`：注释同步到“claim 竞争 / reentrant drive 直接 trap”的当前合同。
   - `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`：补锁 trap 路径与“不再自旋 yield”。
   - 新增三个 run-pass 回归：顺序跨线程 handoff、重入 trap、并发竞争 trap。
3. 关键验证结果：
   - 新增 handoff fixture 单独 `build + run` 输出与 golden 一致。
   - 新增 reentrant / concurrent trap fixture 单独执行均稳定以退出码 `3` 结束。
   - `cargo run -p scoop -- test --fixtures tests/fixtures/build` 通过（`fixtures: ok (17)`）。
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 通过（`fixtures: ok (392)`）。
   - `cargo run -p scoop -- test` 通过（`fixtures: ok (1166)`）。
   - `cargo test --all` 通过。
   - `cargo clippy --all-targets -- -D warnings` 通过。
4. 待执行的收尾动作：
   - 检查 git 工作区，确认只提交本轮相关文件；
   - 提交 commit；
   - 停止，等待下一轮执行 `T4016T8`。
