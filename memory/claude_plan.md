# 执行计划与进度记录

说明：按安全与协作要求，此处记录的是简明执行计划、关键判断依据与进度更新，不写入完整私有推理。

## 初始计划

1. 读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务。
2. 查看最近一次提交，判断是否有与该任务直接相关且明确未完成的问题；若有，将其视为该任务的一部分或在 `TODO.md` 中补成前置依赖。
3. 阅读当前任务涉及的代码、测试与规范说明，只收集完成该任务所必需的上下文。
4. 直接实现当前任务；如果遇到阻塞当前任务的真实缺陷或缺失能力，不绕过，按要求先把前置任务写入 `TODO.md` 并停止。
5. 运行该任务要求的验证，以及必要的回归测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用）。
6. 更新 `TODO.md`：仅在任务真正完成时给任务标题加上 `[DONE]`，并补充完成记录；仅当阶段计划发生变化时更新 `PLAN.md`。
7. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 进度日志

- 本次调用开始：先核对 `TODO.md`、最近一次提交与当前工作区，确认上次记录中的 `P0-T02C` 是否仍是首个未完成任务，以及是否存在未提交的续做内容需要一并收口。
- 已核对：`P0-T02C` 已在 `TODO.md` 标记为 `[DONE]`，且最近提交为 `[P0-T02C] Unbind remaining stable-id test spellings`；当前首个未完成任务已切换为 `P0-T02R：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住`。
- 本次执行目标调整为完成 `P0-T02R`：复核 stable-id 审计 helper、LLVM/pipeline 测试基线与 `.cone`/JSON 健康基线，重跑 P0-T01 / P0-T02 相关验证，并根据 review 结果决定是直接完成该 review 任务，还是先把新发现的阻塞项写成前置任务。
- 已完成 review 搜索：
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中，任务关注的 `@sample.main`、`@sample.classifyValue`、closure adapter / transport / descriptor 旧字符串绑定已无命中。
  - `crates/scoopc/src/llvm/tests.rs` 中与旧 `lambda` / hidden-init / direct-invoke / descriptor 字符串相关的剩余命中，均位于审计 helper、分类器样例或 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests` 的防回流清单；未发现新的行为测试强绑定。
  - `.cone` / JSON 四个健康基线文件中的 dense-id/path 相关命中，均位于 `assert_json_surface_stays_semantic_and_path_free(...)` 的禁止词清单或内部实现字段名（如 `source_paths`），未发现对外 schema surface 泄漏。
- 已完成验证：
  - 定向：`stable_id_audit`、`external_symbol`、`stable_id_source_inventory`、`path_free`、`composite_transport`、`runtime_type_primitives`、`refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry` 全部通过；`closure_step_adapter` 过滤名本身未命中测试名，但其覆盖点已由随后全量 `cargo test -p scoopc` 通过确认。
  - 全量：`LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc` 与 `... cargo clippy -p scoopc --all-targets -- -D warnings` 已通过。
  - grep 审计当前摘要：`module.add_function(...None)` 101 命中、`stable_template_symbol_suffix` 7、`source_path.*decl_span` 5、`scoop.lambda$[0-9]+` 2、`scoop.lambda_resume$[0-9]+` 1、`scoop.lambda_env$[0-9]+` 1、`__schema[0-9]+` / `__k[0-9]+` / `t[0-9]+__` 均为 0；其中剩余 `lambda` 命中仅来自审计测试本身。
- review 结论：P1-P7 后续阶段已有可复用的稳定验证入口：`stable_id_audit_*`/`external_symbol_*` 用于 object/symbol/linkage 审计，`stable_id_source_inventory_*` 用于旧字符串回流防护，四个 `*_path_free` JSON 基线用于 `.cone` / schema path-stability 审计，再辅以 `cargo test -p scoopc` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 做全量收口；未发现需要在 `P1-T01` 前新增的前置修复任务。
- 已更新 `TODO.md`：`P0-T02R` 已显式改为 `[DONE]`，并补齐 review 范围、核心结论、验证结果与 `PLAN.md` / `STABLE_ID.md` 闭合说明。
- 当前仅剩收尾提交：将 `TODO.md` 与本文件一并提交，按要求在完成当前任务后停止。
- 已创建本文件并记录初始计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `P0-T02C：清理 review 发现的剩余 stable-id 敏感 LLVM / pipeline 测试字符串绑定`。
- 已检查最近一次提交：`[P0-T02R] Add prerequisite for remaining test bindings`。该提交与当前任务直接相关，说明 `P0-T02C` 就是为完成 `P0-T02R` 而新增的前置任务，无需再额外拆分。
- 已完成首轮代码修改：
  - `crates/scoopc/src/llvm/tests.rs` 中剩余 closure adapter / class descriptor / object singleton / enum boxed payload 相关断言，已迁移为基于 helper family、局部 typed-descriptor 值、global 角色与 IR 结构的断言。
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中剩余 composite transport / value box / enum payload / closure env / cross-thread resume / argv main wrapper / runtime type primitive 相关断言，已迁移为基于 global 角色、调用关系、typed alloc marker 与函数体结构的断言。
  - `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests` 已扩充，覆盖本轮移除的旧字符串绑定，防止回流。
- 已运行 `cargo fmt`，准备开始任务要求的定向测试与全量验证。
- 定向测试首轮发现 `refactor_class_ctor_uses_concrete_generic_instance_layout` 的新 matcher 过于依赖函数级定位；已改回模块级结构断言（`@scoop_alloc_typed` + `class_type_desc_i8` + concrete class type），保持“不锁死 descriptor symbol 文本”的目标不变。
- 第二轮定向测试进一步确认该路径当前不会显式产出 `class_type_desc_i8` 本地名，而是直接把 descriptor global 传入 `scoop_alloc_typed`；已把断言收敛为“typed alloc + concrete object/payload GEP + 不物化 raw generic 布局”。
- 同轮验证还发现 object singleton receiver 传参会经过额外局部中转，不能假设调用参数名与首次 `load ptr addrspace(1), ptr @slot` 的 SSA 名一致；已把断言改为检查更稳定的事实组合：slot global 角色、成员调用拿到 `addrspace(1)` receiver、且不会直接把 raw global 地址当 receiver。
- 新增验证还确认 enum boxed payload 路径同样不会稳定产出 `enum_boxed_payload_type_desc_i8` / `enum_boxed_payload_obj_ptr` 本地名；已把断言收敛为“boxed payload object type + `rt_alloc_enum_boxed_payload` + payload GEP/materialize 路径”。
- `closure_env_transport` 定向验证显示 closure env / capture box 路径会保留 `rt_alloc_pass_mir_closure_env` / `rt_alloc_pass_mir_capture_box` 这类 typed-alloc 角色 marker，但 `*_desc_i8` 局部名并不稳定；相关断言已改为依赖 alloc marker + field GEP + descriptor/global 角色。
- `runtime_type_primitives` 定向验证显示 `classifyValue` 的静态折叠表现为恒真 `br i1 true` + `phi 7/9`，而不是单一 `ret 7`；相关断言已改为检查该分支/phi 结构，避免继续依赖 callable symbol 文本。
- 全量 `cargo test -p scoopc` 暴露 `refactor_llvm_value_boxing_transport` 与 `refactor_llvm_enum_payload_transport` 仍依赖局部临时名；已改为统一依赖“typed alloc marker + concrete carrier/object type + payload GEP”这组在定向/全量路径下都稳定的结构信号。
- 单独复核 `refactor_llvm_enum_payload_transport` 后，确认 enum -> Any 擦除路径同样不稳定保留 `mir_value_box_desc_i8`；已进一步收敛为“`scoop.mir.value_box$sample_Outer` concrete carrier type + `rt_alloc_mir_value_box` + `mir_value_box_payload_gep`”。
- 验证完成：
  - 定向命令已全部通过：`stable_id_source_inventory`、`closure_step_adapter`、`refactor_class_ctor_uses_concrete_generic_instance_layout`、`object_member_call_uses_gc_managed_singleton_receiver`、`enum_single_field_non_scalar_payload_uses_boxed_variant_path`、`composite_transport`、`closure_env_transport`、`cross_thread_resume_payload_transport`、`runtime_type_primitives`、`refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`。
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc` 已通过。
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings` 已通过。

## 当前执行细化

1. 阅读 `crates/scoopc/src/llvm/tests.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中任务列出的测试，找出仍直接绑定当前 private / descriptor / callable symbol 文本的断言。
2. 复用或补充最小必要的 IR 查询 helper / descriptor 角色识别 helper，把这些测试迁移为基于 helper family、角色、调用关系、布局与结构语义的断言。
3. 扩充 source inventory，阻止本轮清理掉的字符串绑定回流。
4. 运行任务要求的定向测试，再运行全量 `cargo test -p scoopc` 与 `cargo clippy -p scoopc --all-targets -- -D warnings`（如环境允许）。
5. 任务完成后更新 `TODO.md` 的 `[DONE]` 标记与完成记录，必要时同步本文件进度，然后提交一次 git commit 并停止。
