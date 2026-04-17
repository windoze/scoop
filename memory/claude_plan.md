# 执行记录

## 当前目标

本轮只处理 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 执行约束

1. 先检查最近一次提交是否提到需要先修复的既有问题；如有，优先处理。
2. 读取 `TODO.md`、`PLAN.md`，识别第一个未完成任务及其上下文。
3. 如果该任务过大，先拆分任务，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。
4. 实现任务时不接受绕过规范的临时方案；若发现规范缺口或前置问题，必须先在 `TODO.md` / `PLAN.md` 中建前置任务并调整顺序。
5. 完成后必须运行相关验证；若适用，执行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或与变更更匹配的更小验证集。
6. 更新 `TODO.md`、`PLAN.md`、必要文档，并提交 git commit，然后停止。

## 初始步骤

1. 查看最近一次提交信息与改动摘要，确认是否包含待补救问题。
2. 打开 `TODO.md` 与 `PLAN.md`，确认第一个未完成任务。
3. 评估任务规模、依赖与潜在规范阻塞。
4. 根据评估结果实施任务或先做任务拆分。

## 进度日志

- 已创建本文件，后续会在关键节点补充决策、风险与完成情况。
- 已检查最近一次提交：`45992fa [T3014cR] Fix cross-file delegated-property lowering context`。提交说明未新增必须先独立修复的遗留问题。
- 已定位本轮首个未完成任务：`T3014R Review：确认 multi-op handler registration 与 unmatched propagation 已与合同一致`。
- 已阅读 `TODO.md`/`PLAN.md` 中 `T3014`、`T3014a`、`T3014b`、`T3014c` 及 `T3014R` 上下文；当前无需拆分任务，先执行生产代码复审。

## 当前复审重点

1. `dispatch_entries()` 到 runtime handler 注册是否一一对应，避免只注册首个 op-tag。
2. `dispatch_unmatched` / cleanup / handle 退出路径是否会把未匹配 effect 误流入 `handle_done` 正常完成分支。
3. continuation 捕获的 handler stack 是否保留完整动态链，而不是只保留部分 handle 注册信息。
4. effect dispatch / outward propagation 是否仍只依赖统一合同（`op_tag` + `effect_instance_key` + state-machine metadata），没有 shape-based 或 fixture-only 特判。

## 接下来

1. 继续审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`runtime/c/scoop_runtime.c` 与 dispatch contract 构建链。
2. 运行定向验证，再执行 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop --features llvm -- test`。
3. 若未发现新缺口，则更新 `TODO.md` / `PLAN.md` / 本文件并提交；若发现缺口，则先修复或按阻塞规则重排任务。

## 本轮结果

- 已完成生产代码复审，未发现需要新增前置任务或继续修复的 `T3014` 相关缺口。
- 关键结论：
  1. `dispatch_entries()` 与 runtime handler registration 一一对应；每个 dispatch entry 都会独立 push 一个 `ScoopEffectHandlerFrame`，并在 `handle_done` / `handle_propagate` 逆序 pop。
  2. `dispatch_unmatched` 不会流入 `handle_done`；若存在 cleanup，则先执行 cleanup，再进入 `handle_propagate` 向外传播。
  3. continuation 捕获的是完整 handler stack 顶指针，因此 multi-op handle 的动态上下文会被完整保留。
  4. `handle_propagate` 继续复用共享 `emit_ordinary_non_resuming_effect_exit()`，没有 handle-only / shape-based / fixture-only 特判。
- 已执行验证：
  - `cargo test -p scoopc --features llvm multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
  - `cargo test -p scoopc --features llvm same_op_multi_arm_dispatch_ir_reads_effect_instance_key -- --nocapture`
  - `cargo test -p scoop_runtime --test effect_tls -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_custom_indirect.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_op_tag_two_effects_nested_dispatch.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop`（退出码 `23`，符合预期）
  - `target/debug/scoop run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`：仍只停在已跟踪的 `T3017` stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`
- 文档状态已更新：`TODO.md` 已将 `T3014R` 标记完成；`PLAN.md` 已将当前执行顺序推进到 `T3009b`。
