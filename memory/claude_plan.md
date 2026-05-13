# 执行计划

1. 先读取 `TODO.md`，定位第一个标题未标记为 `[DONE]` 的任务；只围绕该任务展开，不做开放式历史问题排查。
2. 读取与该任务直接相关的说明、依赖、验证要求，并检查最近一次提交是否有与该任务直接相关且未完成的说明。
3. 基于任务要求检查相关实现与测试，确认是否可以直接完成；若存在阻塞当前任务的明确前置缺陷或缺失能力，则在 `TODO.md` 中按依赖顺序补充最小前置任务并停止。
4. 若无阻塞，则完整实现当前任务，保持修改尽量小且正确，不引入规避性方案。
5. 运行任务要求的验证，以及必要的相关测试、格式化和 lint，确保结果通过；若发现回归或同类根因问题，一并修复到合理范围。
6. 更新 `TODO.md`：将已完成任务标题前加上 `[DONE]`，补全完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
7. 记录本次执行结果到本文件，然后创建一次 git 提交，提交信息使用当前任务号并准确描述改动。
8. 完成一个任务后立即停止，不继续处理下一个任务。

# 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md` 并确认首个未完成任务为 `P4-T02：把 AbiMangler 接入 exported declaration path，并验证跨路径稳定性`。
- 最近一次提交为 `[P4-T01] Stabilize exported template and instance identity`，未发现额外的未完事项说明；按 `TODO.md` 继续执行 `P4-T02`。
- 已确认当前工作树里存在一批与 `P4-T02` 直接相关的未提交改动，主要集中在 exported declaration path、LLVM audit 测试、pipeline object symbol audit 与 `scoop` 侧 build/fixture 断言；按“恢复当前任务并一起提交”的方式继续完成。
- 已完成实现收口：
  - source-level top-level / materialized plain / effect-lowered plain callable 的 exported declaration path 统一接入 `AbiMangler`。
  - 新增 exported ABI symbol registry，在多条声明路径试图把不同 canonical key 绑定到同一 exported symbol 时显式报 collision。
  - 新增 object external symbol helper 与跨 checkout 根路径、distinct virtual cone 的稳定性/碰撞 smoke 测试。
  - `scoop` 侧 build fixture、CLI pipeline 断言与相关 LLVM fixture 已迁移到 `__scoop_abi0_fun__*` / `__scoop_priv0__*` namespace 语义。
- 已完成验证：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_overloaded_source_level_callables_publish_distinct_abi_symbols -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_vtable_targets_use_abi_mangler_namespace -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_exported_object_symbols_are_path_stable_across_checkout_roots -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc refactor_llvm_user_abi_symbols_stay_disjoint_for_distinct_virtual_cones -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc via_mir_direct_interface_default_call_is_not_reinterpreted_as_itable_dispatch -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_top_level_and_materialized_generic_smoke -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop commands::build:: -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoop --test p7_default_pipeline -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 额外记录：单独尝试 `cargo test -p scoop` 时，命中过一个与本任务无关的既有失败：`crates/scoop/tests/p8_docs_cleanup.rs` 读取缺失文档 `HIR_COMPLETENESS_HANDOFF.md`。该问题不属于 `P4-T02` 的 exported naming / path-stability 变更面，本次未改动该文档路径。
- 已更新 `TODO.md`，将 `P4-T02` 标记为 `[DONE]` 并补全 completion record。
- 下一步：查看最终工作树、创建 `[P4-T02]` 提交并停止。
