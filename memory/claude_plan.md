## 当前执行计划

说明：按要求先记录高层执行计划与进度；这里记录的是可审阅的工作计划与决策，不包含内部逐字推理。

1. 检查最新一次提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，拆分为可独立完成的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行新的第一个子任务。
4. 阅读与该任务直接相关的代码、测试、规格和计划文件，确认实现边界与依赖。
5. 实现该任务；若过程中发现既有缺陷、规格不匹配、实现边界缺失或测试/运行异常，优先修复，或将其作为前置任务插入 `TODO.md` 并更新 `PLAN.md` 后停止。
6. 运行与改动直接相关的测试，再运行必要的更广泛校验，至少包含无警告检查（按仓库情况执行 `cargo clippy --all-targets -- -D warnings` 或等价校验）。
7. 更新进度文档：在 `TODO.md` 标记完成状态，在 `PLAN.md` 反映当前状态与后续安排，并继续更新本文件记录关键进展。
8. 按仓库提交信息风格创建一次提交，然后停止，不继续处理下一个任务。

## 进度

- 已写入初始计划，下一步检查最新提交信息。
- 已检查最新提交：`[T5002b2b1] Align callee resume entry token contract`。提交标题未额外声明需先插队修复的独立既有问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T5002b2b2`：修复 resumed ordinary callee 经 `NestedHandleBoundary` 再次 outward suspend 时 replay chain 丢失。
- 当前执行步骤：
  1. 搜索与 `NestedHandleBoundary`、callee resume replay、pending continuation、resume token 相关实现与回归。
  2. 复现最小失败场景，确认当前错误行为与期望行为的差异。
  3. 基于定位结果做最小正确修复，并补 focused regression。
  4. 运行定向测试、必要的更广验证与 `cargo clippy --all-targets -- -D warnings`。
  5. 更新 `TODO.md`、`PLAN.md`、本文件并提交，然后停止。
- 复现更新：新增 focused run-pass 回归后，当前程序在第一次外层 `k.resume(...)` 时卡住；结合生成 IR 已确认问题出在 fresh continuation materialization：
  - call-site suspend 已会把新的 resume token 存到 frame token slot；
  - 但 non-call suspend（当前已复现到 direct-perform，nested-handle boundary 预期同根因）在 resumed ordinary callee 路径上 materialize fresh continuation 时，没有把当前 TLS 中的 incoming ordinary callee replay token 继承到新 continuation；
  - 后续再 `resume` 这个 fresh continuation 时，runtime 无法恢复 inner ordinary callee replay chain。
- 下一步修复：
  1. 为 codegen 暴露 `scoop_callee_suspend_state_get()` ABI；
  2. 在 state-machine `Suspend` terminator materialize continuation 时，优先捕获 site 自己的 ordinary replay token slot；若该 site 没有 slot，则回退捕获当前 TLS incoming token；
  3. 重新跑 focused fixture，并补成最终需要的 nested-handle immediate-resume 回归形状。
- 已完成子任务拆分：
  - `T5002b2b2a`：non-call suspend materialization 继承当前 incoming ordinary callee token；已完成并补 LLVM 回归。
  - `T5002b2b2b`：nested-handle immediate-resume replay-state 穿过 ordinary callee boundary 的 owner / replay 错位；仍待后续处理。
- 已完成验证：
  - `cargo test -p scoopc resumed_non_call_suspend_ir_captures_current_callee_resume_token_on_materialized_continuation -- --nocapture`
  - `cargo test -p scoopc suspend_ir_stores_callee_resume_token_on_frame_and_replays_via_resume_thunk -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- 当前停止点：
  - 在尝试直接做 nested-handle immediate-resume end-to-end 回归时，确认还存在第二个独立 blocker：legacy replay-state 仍会穿过 ordinary callee boundary 并被误当成 callee resume entry token；因此本次按用户规则将任务继续拆分，只提交已完整收口的第一个子任务。
