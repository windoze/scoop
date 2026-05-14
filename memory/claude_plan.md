## 当前执行计划

说明：此文件记录本次执行的可公开计划、关键判断依据与进度更新，不记录内部私有推理细节。

1. 读取 `TODO.md`，识别首个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交，确认是否存在与该任务直接相关且明确未完成的问题；若有，按用户要求将其并入当前任务范围或补记为前置依赖。
3. 阅读当前任务涉及的代码、测试、规范与任务说明，确认实现边界与验证要求。
4. 直接实现该任务；若发现无法按规范完成的阻塞项，则在 `TODO.md` 中以最小前置任务形式补录并停止，不做规避性实现。
5. 运行与当前任务直接相关的测试/检查；若任务影响面要求更广，则补充运行必要的回归验证与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`：仅在任务真正完成时为任务标题添加 `[DONE]` 并填写完成记录；若出现阻塞，则保持任务未完成并写明新增前置任务。
7. 仅当阶段计划或依赖结构变化时更新 `PLAN.md`。
8. 提交本次所有相关改动，提交信息使用当前任务号。
9. 停止，不继续下一个任务。

## 进度

- 已读取 `TODO.md`，确认首个未完成任务为 `P3-T03`：收口 `StoreMember` continuation route 与 raw function-ref normalization regression。
- 已查看最近一次提交 `87b1da99 [P3-T02] Close ctor and default-arg contract drift`，下一步检查其中提到的 function-value / closure 相关未完事项是否直接属于 `P3-T03` 范围。
- 已确认 `StoreMember` 的 `Ambiguous` 当前已有 MIR verifier 与 codegen 定向测试覆盖；`cargo test -p scoopc refactor_mir_store_member_codegen` 与 `cargo test -p scoopc refactor_mir_member_access_codegen` 通过。
- 已复现 `P3-T03` 的另一半问题：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop` 失败。下一步获取精确错误并修复 `TopLevelRef` / top-level callable value 规范化回归。
- 已定位并修复两类直接阻塞：
  1. nested callable callee（如 `make(1)()` / `choose(mode)()`）在 typecheck 时没有把 callee 推导类型写回 `inferred_expr_tys`，导致 HIR lowering 把 callee 降成 `Any`；现已改为通过 `inputs.infer(...)` 记录该类型。
  2. `String.length()` 这类保留 member-access 形状的 callable 在 HIR lowering / LLVM HIR call lowering 中没有被当成 callable surface 处理；现已为 typed HIR 保留函数类型，并把 builtin member-call short-circuit 前移到 generic callable 分支之前。
- 已为 materialized MIR 增补顶层 value 类型索引，使 effect-facts 能为 `topNamed` / `topPatternF` / `topFp` 这类顶层 callable value 构建 surface contract，而不再依赖 generic MIR root 留在 materialized snapshot 里。
- 已新增 typed-HIR 回归测试，覆盖 nested callable callee 类型保留与 top-level immutable receiver closure side table 形状。
- 已完成验证：
  - `refactor_mir_member_access_codegen`、`refactor_mir_store_member_codegen`
  - `typed_hir_preserves_function_typed_nested_call_callee`
  - `typed_hir_top_level_immutable_receiver_closure_keeps_length_as_call_in_side_table`
  - `materialized_mir_closure_private_symbols_use_stable_hash_namespaces`
  - `callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir`
  - `callable_value_pattern_binder_receiver_named_args_fixture_codegen_succeeds`
  - `higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval`
  - `higher_order_effectful_function_value_uses_schema_aware_carrier_adapter`
  - fixture：`top_level_callable_value_call_basic.scoop`、`callable_value_pattern_binder_receiver_named_args_basic.scoop`、`assignment_places.scoop`
  - `codegen_gap_inventory`、`pipeline_gap_audit`
  - `cargo clippy --all-targets -- -D warnings`
- 额外复核：`cargo test -p scoopc llvm::tests -- --nocapture` 中与本任务直接相关的历史失败已消失；剩余 3 个失败对齐后续 `P4` / explicit-root-frame 任务，不阻塞 `P3-T03` 完成。
- 下一步：提交本任务改动并停止。
