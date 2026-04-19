## 当前目标

- 本轮只完成 `TODO.md` 中首个未完成任务 `T4009b`，完成后停止。
- 先确认上一轮遗留的真实回归是否已经被当前工作树中的修复解决。
- 若仍有 spec mismatch 或实现缺口，必须先把问题定位清楚；如果无法在本轮直接修完，则按要求更新 `TODO.md` / `PLAN.md` / 本文件并停止。

## 已知上下文

- `T4009b` 的主体实现已经落在当前工作树中，包括 runtime task poll API、lowering、LLVM codegen、文档与 fixture。
- 本轮开始时，上一轮遗留的 `async_fun_task_runtime_basic.scoop` 回归已经在现有代码里被修好；最小验证一开始即通过。
- 随后在全量 fixture 中暴露出新的真实回归 `tests/fixtures/run-pass/continuation_resume_continuation.scoop`：
  - 现象是 `ok.resume(ik)` 恢复外层 continuation 后，`innerK.resume(42)` 整段被吞掉，只打印 `got_inner_k` 和 `inner_resumed`，缺少 `inner_got / 42`。
  - 根因不是 continuation transport 本身坏了，而是 unified state machine emitter 对 `BindLocal/DeclareAnonymousVal` 的 initializer override 判断过宽：它把所有 `Handle` 都当成“init 已由前置 state actions 求值”，从而错误消费了前一个 `println(...)` 留下的 `last_value = Unit`。
  - planner 实际上只会为“真的向外 suspend 的 initializer”提前拆 state actions；对 self-contained nested handle（例如 `try { innerK.resume(42) } catch ...`）不会拆动作，仍应在 emitter 中正常 `codegen_decl_initializer_expr(...)`。

## 执行计划

1. 先读取并核对当前仓库状态：
   - 查看 `memory/claude_plan.md` 是否已有内容并与当前计划对齐；
   - 查看 `TODO.md` / `PLAN.md` / `git status` / 最新 commit，确认 `T4009b` 仍是首个未完成任务，且没有新引入的前置问题。
2. 收集上一轮仍在运行或待确认的测试结果；若 session 已失效则重跑最小验证：
   - `async_fun_task_runtime_basic.scoop`
   - `task_poll_step_manual_basic.scoop`
   - 两个 LLVM/IR 相关单测
3. 若 `async_fun_task_runtime_basic.scoop` 仍失败，集中检查：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - `crates/scoopc/src/llvm/codegen/control_flow.rs`
   - `crates/scoopc/src/llvm/codegen/mod.rs`
   - `crates/scoopc/src/llvm/codegen/stmt.rs`
   - 重点确认 block-local return 在 async ready local 初始化中的 lowering / planning / emitting 是否正确转化为 continuation，而不是函数返回。
4. 修复后执行分层验证：
   - 最小回归测试
   - `cargo test --all`
   - `cargo run -q -p scoop -- test`
   - `cargo run -q -p scoop_tools -- spec-fixtures check`
   - `cargo clippy --all-targets -- -D warnings`
5. 若全部通过，更新文档状态：
   - `TODO.md` 将 `T4009b` 标记完成
   - `PLAN.md` 记录完成情况
   - `memory/claude_plan.md` 记录关键步骤完成情况
6. 提交本轮变更并停止，不继续下一个任务。

## 执行约束

- 全程不依赖 workaround。
- 所有文件修改只用 `apply_patch`。
- 如果发现新的真实阻塞且本轮无法正确解决，必须调整 `TODO.md` / `PLAN.md` 并提交后停止。

## 关键进展

- 已确认最新提交 `[T4009a3] Remove executor task codegen special-cases` 没有额外口头挂出的前置 issue；`TODO.md` 首个未完成项仍是 `T4009b`。
- 已完成真实修复：
  - `state_machine_plan.rs` 为 `HandleStateOp::BindLocal` / `DeclareAnonymousVal` 新增 `init_from_last_value` 元数据，由 planner 显式标记“initializer 是否确实由前置 state actions 求值”。
  - `state_machine_emitter.rs` 已删除那套与 planner 不一致的 AST 猜测逻辑，改为只在 `init_from_last_value=true` 时消费 `last_value`。
  - 这同时保住了 async task `__task_ready_value` local-return 修复，又恢复了 `continuation_resume_continuation` 里 self-contained nested handle initializer 的正常执行。
- 已完成最小验证：
  - `cargo test -q -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/task_poll_step_manual_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
- 已完成全量验证：
  - `cargo fmt --check`
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo run -q -p scoop -- test` -> `fixtures: ok (1071)`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步只剩：
  - 把 `TODO.md` / `PLAN.md` 标记为 `T4009b` 已完成
  - 检查最终 diff 与 `git status`
  - 提交 commit 并停止
