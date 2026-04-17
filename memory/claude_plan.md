# 本轮执行计划

## 目标
- 按 `TODO.md` 的顺序，只完成第一个未完成任务，然后停止。
- 在开始正式实现前，先检查最新提交是否提到已有问题；若有，先修复这些问题。

## 初始步骤
1. 查看最新一次提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务依赖。
4. 如果该任务过大，则把它拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
5. 实现当前应执行的第一个任务或子任务。
6. 运行相关测试、格式化、lint，至少覆盖：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时运行更聚焦的测试命令以缩短迭代
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮完成的任务标记为已完成
   - 在 `PLAN.md` 中记录完成情况与后续影响
   - 按需要更新本文件，记录关键进展或计划调整
8. 使用清晰的提交信息提交本轮修改。
9. 停止，不继续处理下一个任务。

## 执行原则
- 不接受规避方案、临时垫片或只改夹具的修复。
- 一旦发现规范不匹配、缺失特性、错误实现或依赖缺口，必须先在 `TODO.md` / `PLAN.md` 中显式建账并调整顺序。
- 不回退与当前任务无关的现有改动。

## 当前状态
- 已检查最新提交：`abff84b249f3ca115af2c779ccfcbf9f8b827faf`（`[T3014bR] Restore hidden raise effect-instance key contract`）
- 已读取 `TODO.md`
- 已读取 `PLAN.md`
- 已确定本轮具体任务：`T3014c`

## 当前任务判断
- `TODO.md` 中第一个未完成项是 `T3014c [TODO] 修正 delegated-property observable 回调内 Raise.raise(...) 的 state-machine effect_instance_key 缺口`。
- `PLAN.md` 记录的最新首个真实失败点也是 `delegated_property_observable_raise_does_not_poison_mutex.scoop`，报错为 `UnsupportedMainBody { kind: "state machine perform effect instance key" }`。
- 因此，最新提交提到的既有问题已经继续前移并显式建账为 `T3014c`；本轮先修这个问题，符合“先处理最新提交提到的既有问题，再执行第一个未完成任务”的要求。

## 已确认根因
- 不是 runtime dispatch/ABI 缺 `effect_instance_key`，而是 delegated property 的标准 delegate inline body 没进入 typed side table。
- 具体来说：`typecheck/expr/entry.rs` 之前会直接跳过带 `delegate` 的属性表达式，导致 `observable` callback 内 `Raise.raise(...)` 从未写入 `inferred_performed_effect_tys`。
- HIR lowering 随后仍会把 callback body inline 到 delegated-property assign lowering，但 `Perform.effect_ty` 只能退化成 `Any`，最终 unified state-machine `emit_perform_op` 无法求出统一 `effect_instance_key`。
- 尝试把整个 delegate 调用按普通 call typecheck 会误把 `observable` callback 当成纯 lambda，从而打坏现有 run-pass 语义；因此本轮正确修复不是“typecheck 整个 delegate 调用”，而是只 typecheck 标准 delegates 实际会被 lowering inline 的表达式：
  - `lazy` 的 initializer body
  - `observable` / `vetoable` 的 initial 表达式
  - `observable` / `vetoable` 的 callback body

## 已完成修改
1. 在 `crates/scoopc/src/typecheck/expr/entry.rs` 中新增标准 delegated-property inline 表达式检查：
   - 为 `lazy` initializer body 提供 expected-context inference
   - 为 `observable` / `vetoable` 的 initial 表达式提供 expected-context inference
   - 为 `observable` / `vetoable` callback body 在 `old/new` = property type 的局部语境下做 expected-context inference
2. 未采用“把整个 delegate 调用走普通 call typecheck”的错误方案，避免把 effectful callback 误判为违反纯 lambda 签名。
3. 在 `crates/scoopc/src/hir/lower/mod.rs` 新增 typed lowering 回归 `lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback`，锁定 observable callback 内 `Raise.raise(7)` 的 `Perform.effect_ty` 为 `Raise<Int>`。

## 已完成验证
- `cargo test -p scoopc lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback -- --nocapture`
- `cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables -- --nocapture`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
- `cargo fmt --all`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run -p scoop --features llvm -- test`

## 验收结论
- 目标 fixture 已恢复通过，不再报 `state machine perform effect instance key`。
- 全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- 完整 LLVM fixture suite 已不再首先停在 `delegated_property_observable_raise_does_not_poison_mutex.scoop`，而是继续前移到已跟踪的 stale `EXPECT: fail`：`effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）。
- `TODO.md` 已将 `T3014c` 标记为完成；`PLAN.md` 已更新为下一项 `T3014cR`。

## 当前剩余步骤
1. 检查工作区 diff，确认只包含本轮相关修改
2. 提交 git commit
3. 停止

## 细化执行步骤
1. 阅读 `T3014c` / `T3014cR` 在 `TODO.md` 附近的描述，明确验收标准。
2. 定向复现 `delegated_property_observable_raise_does_not_poison_mutex.scoop`，确认失败栈与触发路径。
3. 检查 delegated-property observable callback 的 lowering / typecheck / codegen 链路，定位为什么 state-machine perform 缺少 `effect_instance_key`。
4. 实现修复，并补充最小回归测试，优先锁定：
   - delegated-property observable 回调中的 `Raise.raise(...)` HIR `Perform.effect_ty`
   - LLVM lowering 中 `effect_instance_key(effect_ty)` 的可达性
5. 运行定向测试，再运行全量 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`，必要时补跑 fixture。
6. 更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T3014c` 完成并记录验证结果。
7. 提交 git commit 后停止。
