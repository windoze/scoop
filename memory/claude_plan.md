## 当前执行计划

1. 已完成：读取 `TODO.md`，确认首个未完成任务是 `CG-T07S0a11`；最近一次提交 `[CG-T07S0a10]` 仅说明前一个 blocker 已修复，并把当前任务登记为新的前置阻塞，没有额外未完成子项需要先补录。
2. 已完成：复现 `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop` 的 build 失败，确认 `MemberAccessMetadata.resolved = Value { fqn: "Outer.Nested" }` 仍被送进 `pass MIR member field target`，问题发生在 MIR/effect-refactor 对 resolved static value 的 contract 判定遗漏了 object singleton value。
3. 已完成：阅读 nested object、named companion、singleton once-init、value-ref/member access lowering 相关实现与测试，确认 HIR codegen 已把 object singleton value 当作 top-level value/ref 处理，而 MIR/effect-refactor helper 未把 `object_inits.contains_key(fqn)` 纳入同一 contract。
4. 已完成：以最小改动修复 authoritative value-ref / member contract 的消费路径，在 `crates/scoopc/src/llvm/codegen/mir_body.rs` 与 `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs` 中把 object singleton value 纳入 resolved static value 判定，并让其 codegen type 归类为 `CgTy::Ref`。
5. 已完成：补充 `llvm::tests::production_codegen_lowers_nested_object_and_named_companion_value_access` 回归测试，覆盖 nested object / named companion 值引用与成员访问继续走 singleton once-init/backing 主线。
6. 已完成验证：`cargo fmt`、`cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver`、`cargo test -p scoopc production_codegen_lowers_nested_object_and_named_companion_value_access`、`cargo run -p scoop -- build tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop -o /tmp/object_companion_value_named_nested_init_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。默认 full-suite 已越过当前 fixture，新的下一处 blocker 为 `tests/fixtures/run-pass/operator_overload_struct_basic.scoop`。
7. 已完成文档同步：`TODO.md` 已将 `CG-T07S0a11` 标记为 `[DONE]`，并新增下一条前置任务 `CG-T07S0a12` 记录 `operator_overload_struct_basic.scoop` 的新 blocker。接下来只剩提交本次改动并停止。

## 执行约束

- 不用规避性 workaround；遇到阻塞当前任务的真实缺口时，必须先修复或在 `TODO.md` 中登记为前置任务。
- `TODO.md` 是任务顺序与完成状态的唯一真源；`PLAN.md` 只在阶段级计划变化时更新。
- 任务完成必须以标题加上 `[DONE]` 为准，不能只写完成记录。
- 修改尽量最小化，并保持实现、测试、文档与验证一致。
