# 本轮执行计划

注意：按系统安全约束，这里记录的是可公开的高层依据与执行计划，不包含完整私有推理。

## 目标

完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 执行步骤

1. 检查最新一次 Git 提交信息，确认是否提到需要先修复的既有问题；若有，先处理这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 与相关代码，判断该任务是否可直接完成。
4. 如果任务过大或被前置问题阻塞：
   - 将任务拆分为更小的子任务，更新 `PLAN.md`。
   - 按依赖关系更新 `TODO.md` 的顺序与描述。
   - 本轮只执行拆分后的第一个子任务，或在阻塞情况下只提交计划调整。
5. 实现当前目标任务，保证实现符合规范，不引入临时性绕过方案。
6. 运行相关测试，并尽可能补充或更新测试。
7. 更新文档状态：
   - 在 `TODO.md` 标记本轮任务完成，或在阻塞时重排任务并保留为待办。
   - 在 `PLAN.md` 记录当前状态、依赖关系与后续安排。
   - 视进展同步更新本文件。
8. 检查代码质量，至少运行与改动相关的验证；若适用，运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
9. 使用清晰的提交信息提交本轮变更。
10. 停止，不继续下一个任务。

## 当前状态

- 已创建本轮计划文件。
- 已检查最新提交：提交标题为 `[T4008cP] Support multi-payload ordinary perform transport`，未额外声明需先单独处理的新遗留 issue。
- 已阅读 `TODO.md` / `PLAN.md`。
- 已定位首个未完成任务：`T4008cS`，目标是支持 effect state-machine 的多实参 `perform` payload lowering。
- 已阅读 `T4008cS` 相关代码，确认普通 `perform` 已有 `codegen_perform_payload_value(...)` 多 payload 打包逻辑，而 state-machine `emit_perform_op(...)` 仍停留在 0/1 payload 分支。
- 已用临时探针 `/tmp/t4008cs_probe.scoop` 复现当前错误：LLVM 阶段报 `state machine perform arity`。
- 结论：当前没有新的前置 blocker，可直接实现，计划复用现有 tuple transport helper 到 state-machine perform 路径，并补 run-pass / LLVM 回归。
## 2026-04-19 本轮续做记录

### 当前目标
- 继续完成 `TODO.md` 中第一个未完成任务 `T4008cS`：支持 effect state-machine 的多实参 `perform` payload lowering。
- 本轮不会推进后续任务，只会完成 `T4008cS` 或在发现真实前置阻塞时重排 `TODO.md` / `PLAN.md` 后停止。

### 已知状态
- 前一轮已经完成两部分工作：
  - state-machine direct `perform` 已改为复用普通 `perform` 的 tuple payload transport；
  - 已补充一个 run-pass fixture 和一个 LLVM 单测，且定向单测已通过。
- 当前剩余问题不是 tuple transport 本身，而是 state-machine resume rewrite 对跨 state 的表达式 consumer 覆盖不完整。
- 已确认最小复现中，`perform` 位于 `if` 表达式内部时，resume 后恢复值没有被 merge state 消费，导致结果错误地回退到 `0`。

### 本轮执行计划
1. 继续检查 `state_machine_plan.rs` 中 resume fragment materialization / rewrite 路径，确认跨 state `if` consumer 丢失恢复值的具体入口。
2. 修改 state-machine plan 或相邻 lowering 逻辑，使 resume slot 的重写能够覆盖 `if` 等会把 consumer 放到后继 state / merge state 的情形。
3. 用最小 probe 与新增 fixture 复验：
   - `/tmp/t4008cs_probe3.scoop`
   - `tests/fixtures/run-pass/effect_state_machine_multi_payload_basic.scoop`
4. 补充或调整测试，确保多 payload lowering 与 resume consumer rewrite 一起被覆盖。
5. 跑格式化、相关测试、全量测试与 `clippy`。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 git commit 后停止。

### 约束与执行原则
- 不使用 workaround，不靠 fixture 特判掩盖语义问题。
- 若发现这是独立前置缺陷，必须把缺陷显式写回 `TODO.md` 并调整依赖顺序。
- 编辑继续使用 `apply_patch`，不回滚用户已有改动。

### 本轮已完成
- 已完成 `T4008cS`。
- `state_machine_emitter.rs` 中的 direct `perform` 已改为复用普通 `perform` 的共享 tuple payload transport，state-machine 路径不再只接受 0/1 payload。
- `state_machine_plan.rs` 已补齐 resume rewrite 对跨 state `if` consumer 的覆盖：当 `perform` 位于 `if` 表达式内部，而实际 consumer 落在后继 merge state / goto 链共享 state 中时，resume 路径会克隆并改写专用 consumer state，从而保留普通分支的原始语义，同时让 resumed value 正确流入外层绑定。
- 已新增回归：
  - source-plan 单测：`source_plan_clones_if_expr_merge_consumer_for_resume_path`
  - LLVM 单测：`state_machine_multi_payload_perform_uses_tuple_transport`
  - run-pass fixture：`tests/fixtures/run-pass/effect_state_machine_multi_payload_basic.scoop`

### 验证结果
- 最小 probe `/tmp/t4008cs_probe3.scoop` 现输出 `7`，退出码 `8`。
- `tests/fixtures/run-pass/effect_state_machine_multi_payload_basic.scoop` 现输出恢复为预期，退出码 `177`。
- 既有 `effect_resume_if_then_branch_single_perform.scoop` 与 `effect_resume_if_else_branch_single_perform.scoop` 继续通过。
- 已通过：
  - `cargo fmt --check`
  - `cargo test --all`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1062)`）
  - `cargo clippy --all-targets -- -D warnings`

### 待收尾
1. 更新 `TODO.md` / `PLAN.md` 已完成状态并确认 diff。
2. 提交 git commit。
3. 停止，等待下一轮从 `T4008c2` 继续。
