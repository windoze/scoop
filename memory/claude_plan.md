# 本轮执行记录（2026-04-18）

## 本轮任务

- 按 `TODO.md` 顺序执行第一项未完成任务：`T3014bR`。
- 目标：只审查生产代码，确认 hidden-suspend runtime-error raise lowering 已按统一 `effect_instance_key` 合同收口；若成立则更新文档并提交，不推进 `T3014c`。

## 已完成步骤

1. 检查最新提交 `2c006d7befbfd1a559d89c2889ad8d9f8411d448`，确认其内容是在记录既有 blocker（hidden raise key blocker），不存在必须先插入本轮处理的新提交说明问题。
2. 读取 `TODO.md` / `PLAN.md`，确认当前第一项未完成任务为 `T3014bR`，`T3014c` 位于其后，因此本轮不应跳过 review 直接去做 delegated-property 修复。
3. 复审 production code 中的 `effect_instance_key` 合同链路：
   - `crates/scoopc/src/llvm/codegen/mod.rs`：`effect_instance_key(effect_ty)` 对 `Raise<RuntimeError>` 固定返回 `EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR`。
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`：ordinary `codegen_perform_expr` 使用 `effect_instance_key(effect_ty)`；runtime Raise helper `emit_raise_runtime_error_variant` 直接写入同一个常量。
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：unified `emit_perform_op` 也统一调用 `effect_instance_key(effect_ty)`；dispatch 读 TLS `effect_instance_key` 后，通过 `matching_effect_instance_keys_for_handled_effect()` 做 arm 匹配。
4. 复审 hidden-suspend class/object-init 链路：
   - `crates/scoopc/src/hir/lower/util.rs`：`collect_object_inits()` / `collect_class_inits()` 现已接收 `typecheck_types` 并传入通用 `HirLoweringSetup`。
   - `crates/scoopc/src/typecheck/expr/entry.rs`：`check_file_exprs()` 现已覆盖 `ast::Item::Object`，object property initializer 与 `init {}` block 会写回 `inferred_performed_effect_tys`。
   - `crates/scoopc/src/hir/lower/expr.rs`：object/class init 最终仍通过 `typechecked_performed_effect_ty()` 读取 side table 并生成通用 `ExprKind::Perform { effect_ty, .. }`，没有 hidden-suspend helper / ctor / object property / runtime-error-only 特判。

## 验证结果

- `cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_effect_ty_in_init_side_tables -- --nocapture`：通过。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`：通过；输出表现为 `boom.init -> caught -> 10 -> done`，说明 ctor helper raise 仍由 outer `try/catch` 捕获。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`：通过；对象属性 hidden-suspend helper 路径恢复正常。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_helper_object_property_basic.scoop`：通过；hidden-suspend helper 与 handle 路径兼容。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_same_op_multi_arm_dispatch_effect_instance.scoop`：退出码 `23`，保持 same-op multi-arm dispatch 的 key 合同验证通过。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo run -p scoop --features llvm -- test`：首个失败点仍为 `tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`，报 `UnsupportedMainBody { kind: "state machine perform effect instance key" }`；未倒回 hidden-suspend runtime-error raise 路径。

## 结论

- `T3014bR` 可以判定完成。
- ordinary `perform` lowering、runtime Raise helper、state-machine perform lowering 与 dispatch 当前共享同一套 `effect_instance_key` 合同。
- hidden-suspend class/object-init 修复是“恢复 typed side table 到通用 lowering 链路”，不是新增 ordinary-path fallback。
- `T3010b2b0a0` 已锁定的语义仍成立：当前 callee 立即终止，outer `try/catch` 命中，caller tail 不继续执行。

## 下一步

- 更新 `TODO.md` / `PLAN.md`，将 `T3014bR` 标记为完成，并把当前第一项未完成任务推进到 `T3014c`。
- 提交本轮改动后停止。
