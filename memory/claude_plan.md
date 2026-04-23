# 执行计划

1. 先检查最新一次提交的信息，确认是否提到了需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和当前计划状态。
4. 如首个未完成任务过大或存在前置缺口，先把它拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
5. 对当前应执行的首个任务进行实现，必要时同时修复在检查、测试、实现过程中发现的既有问题。
6. 运行与该任务直接相关的测试，并补充必要验证；如果发现失败或规格不匹配，先修复问题，再继续验证。
7. 完成后更新 `memory/claude_plan.md`、`TODO.md` 与 `PLAN.md`，标记已完成事项，记录任何计划调整。
8. 检查工作区改动，确保没有引入编译、格式化或 lint 警告；在合理范围内运行所需检查。
9. 为本次完成的单个任务创建一次 git 提交，然后停止，不继续下一个任务。

# 当前状态

- 已创建执行计划文件。
- 已检查最新提交：最新提交 `fe85c18f` 仅更新计划，未额外声明必须先修的既有问题。
- 已读取 `TODO.md` / `PLAN.md`。
- 已定位首个未完成任务：`T4016T4`。

# 针对 T4016T4 的执行细化

1. 核对 `T4016T4` 的验收标准，确认本轮只需要先同步设计文档、最小规格草案和实现注释，不直接实现 claim-bit。
2. 检查 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`sysroot/core.scoop` 中所有仍把 `Pending` 解释为 drive contention，或把并发/重入 `step()` 误用视为可恢复行为的叙述。
3. 修改上述文档与注释，统一到新的合同：
   - `Task` 不是可共享并发 drive 的 thread-safe shared object；
   - 不支持 shared subtask / multiple parents；
   - `Pending` 只表示尚未完成且当前无法继续推进；
   - 并发 / 重入 `step()` 或对外观察到 `Running` 属于 executor/driver 误用，应直接 trap；
   - cross-thread 只允许顺序 handoff，不允许同时 drive。
4. 复查仓库内相关文案，确认不再把旧语义写成稳定合同。
5. 运行必要验证，随后更新 `TODO.md`、`PLAN.md` 和本文件，最后提交并停止。

# 已完成进展

- 已完成 `SCOOP_TASK.md` 的合同收口：
  - step algorithm 已改为“exclusive drive ownership + trap-on-contention”；
  - synchronization design 已明确 shared subtask / multiple parents 不在 core 范围内；
  - `Pending` 已明确为真实 not-ready，而非 contention 结果；
  - cross-thread 只允许顺序 handoff。
- 已同步最小规格草案与实现注释：
  - `SCOOP_FULL_SPEC.md`
  - `SCOOP_RUNTIME.md`
  - `sysroot/core.scoop`
  - `sysroot/task.scoop`
- 已完成验证：
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新任务状态：
  - `TODO.md` 已将 `T4016T4` 标记为 `[DONE]`
  - `PLAN.md` 已将下一步推进到 `T4016T5`
