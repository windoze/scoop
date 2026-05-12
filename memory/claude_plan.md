# Claude Plan

## 初始执行计划

1. 读取 `TODO.md`，定位首个标题未标记 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题需要一并处理或在 `TODO.md` 中登记为前置任务。
3. 阅读该任务涉及的代码、测试、约束和依赖，确认是否可以按当前任务边界直接完成。
4. 若存在阻塞当前任务的真实前置问题，先最小化更新 `TODO.md` / `PLAN.md`（仅在阶段计划变化时）并记录阻塞，再提交并停止。
5. 若无阻塞，直接实现当前任务，保持改动最小且满足规范，不采用变通方案。
6. 运行该任务要求的验证，以及必要的 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`。
7. 更新 `memory/claude_plan.md` 记录关键进展；完成后把任务在 `TODO.md` 中标记为 `[DONE]`，补全完成记录。
8. 如阶段计划未变化则不改 `PLAN.md`；最后按仓库提交风格创建一次提交，然后停止，不继续下一个任务。

## 说明

- 这里记录的是可审计的执行计划与决策，不包含内部推理细节。

## 当前任务确认

- `TODO.md` 中首个未完成任务为 `P0-T02R：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住`。
- 最近一次提交为 `[P0-T02B] Unbind callable symbol test spellings`，与本次 review 直接相关；本次工作将基于该提交结果复核 P0 审计脚手架与测试基线是否已闭合。

## 当前执行细化

1. 复核 `crates/scoopc/src/llvm/tests.rs`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 及四个 `.cone`/JSON 健康基线文件，确认审计 helper、source inventory 与行为测试是否仍残留旧字符串绑定。
2. 结合代码搜索，检查 review 关注的旧命名样式（如 `scoop.lambda$0`、`__schema3`、`__scoop_object_init__...` 等）是否仍被测试当作正确性锚点。
3. 重跑 P0-T01/P0-T02 要求的关键测试与 grep 审计，确认 object/symbol/path-stability/dense-id 泄漏审计入口可复用。
4. 若发现阻塞 P1 的真实问题，优先修复或把最小前置任务写入 `TODO.md`；若未发现阻塞，则把 `P0-T02R` 标记完成并补全完成记录。
5. 运行必要格式化/静态检查，提交本次 review 结果后停止。

## 当前发现

- `crates/scoopc/src/llvm/tests.rs` 中仍残留若干直接绑定当前 private / descriptor symbol 的行为测试，例如：
  - `effectful_closure_dynamic_fallback_uses_schema_aware_carrier_adapter`
  - `higher_order_effectful_function_value_uses_schema_aware_carrier_adapter`
  - `refactor_class_ctor_uses_concrete_generic_instance_layout`
  - `object_member_call_uses_gc_managed_singleton_receiver`
  - `enum_single_field_non_scalar_payload_uses_boxed_variant_path`
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中仍残留若干直接绑定当前 transport / descriptor / callable symbol 的行为测试，例如：
  - `refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals`
  - `refactor_llvm_value_boxing_transport`
  - `refactor_llvm_enum_payload_transport`
  - `refactor_llvm_closure_env_transport`
  - `refactor_llvm_cross_thread_resume_payload_transport`
  - `refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
  - `refactor_llvm_runtime_type_primitives`
- 这些断言会在后续 P3/P4 调整 private naming source、transport type 名称和 user ABI surface 时制造非语义噪音，因此当前不能把 `P0-T02R` 视为完成。

## 决策

1. 不在本次 invocation 内继续扩大实现范围去顺手清理整类测试绑定。
2. 在 `TODO.md` 中新增最小前置任务 `P0-T02C`，专门清理 review 发现的剩余 stable-id 敏感 LLVM / pipeline 测试字符串绑定。
3. 将 `P0-T02R` 的依赖更新为 `P0-T02C`，保持任务顺序真实反映当前阻塞关系。
4. 本次提交只记录 review 发现与任务重排，不改 `PLAN.md`，因为阶段级计划并未变化。
