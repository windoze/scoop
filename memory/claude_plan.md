# 当前执行计划（2026-04-24）

## 任务背景

- 本轮目标仍然是完成 `TODO.md` 中首个未完成任务 `T4017d`：将 ordinary direct / closure / funptr effectful call 切换到显式 `ctx + outcome` internal ABI，然后停止。
- 已检查上一轮结论：最新提交 `710db77 [T4017c] Introduce explicit effect contract abstractions` 没有在提交信息中声明一个必须先修的既有 issue，因此继续推进 `T4017d` 本身。
- 已有实现已经把 ordinary non-state-machine call path 的 direct / closure / funptr 边界部分迁到显式 `ctx/outcome`；但全量 fixture 暴露出真实回归，说明当前任务尚未完成。

## 已确认的回归与根因

- `cargo run -p scoop -- test` 触发 `tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop` 失败。
- 失败表现：`try/catch` 没有捕获到 helper 内部构造函数抛出的 outward effect，调用点后续路径错误地继续沿 inactive 分支执行。
- 根因不是 helper 自身没有提前返回；而是 state-machine emitter 的 ordinary call suspend-site fresh path 仍然使用 TLS `scoop_effect_is_active` 作为 suspend 判断来源。
- 现在 direct wrapper 会在返回前把 TLS 中的 outward signal consume 到显式 `ScoopEffectOutcome`，因此 fresh path 再读 TLS 会错误地看到 inactive。
- 结论：`T4017d` 不仅要覆盖 ordinary 非状态机 caller，还必须把 state-machine emitter 中 ordinary direct / closure / funptr call 的 fresh suspend path 一起切换到显式 outcome。

## 这轮执行步骤

1. 阅读并定位 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中 ordinary call suspend-site fresh path 的实现，确认 direct / closure / funptr 分支当前如何判断 active/inactive。
2. 设计并实现统一修复：
   - 对已经迁移到显式 outcome contract 的 ordinary direct / closure / funptr call，fresh path 改为读取 outcome/tag，而不是再读 TLS active。
   - 对仍未迁移的 suspend 边界保持原有 TLS 逻辑，避免误改 `T4017f` 范围。
3. 用最小但完整的方式补测试：
   - 先复现并验证 `class_init_hidden_raise_helper_try_catch_basic.scoop`。
   - 如有必要，给 LLVM 单测补充 state-machine fresh path 使用显式 outcome 的断言。
4. 跑验证：
   - 相关 targeted tests
   - `cargo run -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若验证通过，更新 `TODO.md`、`PLAN.md`、本文件，标记 `T4017d` 完成，并提交一次 git commit，然后停止。

## 当前约束与判断

- 这不是新的前置任务，而是 `T4017d` 既有实现未覆盖完整调用路径导致的真实回归，因此应直接修复，不应把任务拆出去另起一个 TODO。
- 不允许通过缩窄 fixture、绕开 state machine path、保留 TLS probing workaround 来继续推进。
- 需要持续更新本文件，记录关键实现点、验证结果和是否完成任务。

## 已完成的关键实现（进行中）

- 已在 `crates/scoopc/src/llvm/codegen/mod.rs` 增加 state-machine suspend-call outcome 捕获通道：
  - `active_suspend_site_effect_outcome_capture`
  - `suspend_site_explicit_effect_outcomes`
- ordinary direct / closure / funptr call 在生成显式 outcome boundary 后，会把当前 `SuspendCall` 对应的 `outcome_slot` 记录下来，供 state-machine terminator 使用。
- 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 修复 fresh path：
  - `HandleStateOp::SuspendCall` 在求值期间安装当前 site 的 outcome capture 上下文。
  - `UnifiedStateTerminator::Suspend` 若发现该 site 有显式 outcome slot，则改为读取 outcome tag 判定 propagating，而不是调用 `scoop_effect_is_active`。
  - active 分支会先 `publish_effect_outcome_from_slot(...)`，把 wrapper/边界 consume 过的 signal 回写到 TLS，再继续原 suspend / handler-dispatch 主线。

## 当前验证结果

- `cargo fmt --all` 已通过。
- `tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop` 已恢复 golden 输出：
  - `main_before_call`
  - `helper_before_ctor`
  - `boom.init`
  - `caught`
  - `10`
  - `done`
- 已新增 state-machine IR 测试 `direct_suspend_call_fresh_path_uses_explicit_outcome_instead_of_tls_probe`，锁定：
  - fresh path 读取 outcome tag
  - active 分支 publish outcome 回 TLS
- 已同步更新既有测试 `indirect_if_branch_callee_keeps_handle_call_site_active_dispatch` 的断言，使其检查新的 outcome-based contract。

## 剩余步骤

1. 更新 `TODO.md` / `PLAN.md` / 本文件，把 `T4017d` 标记为完成、把下一步切到 `T4017e`。
2. 检查 git diff，确认只包含本轮 `T4017d` 实现与测试/计划文件更新。
3. 提交 `T4017d` 完成 commit，然后停止。

## 最终验证结论

- `cargo run -p scoop -- test`：通过，`fixtures: ok (1169)`。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- 期间额外修复并同步更新了一条既有 state-machine LLVM 单测：
  - `async_task_resume_replay_ir_terminates_step_fn_on_active_effect`
  - 原断言仍锁旧的 TLS-probing 变量名；现已改为 outcome-based 断言，并与新实现一致。

## 任务状态

- `T4017d` 已完成。
- 下一轮首个未完成任务应为 `T4017e`：把 continuation replay、`callee_suspend_state` 与 `pending_continuation` 迁出 TLS，收口到 `frame + ctx + signal/resume token`。
