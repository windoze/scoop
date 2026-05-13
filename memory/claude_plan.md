# 本次执行计划

说明：不记录不可验证的内部思维细节；这里记录可执行计划、关键判断依据、进度与变更。

## 初始计划

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务；以它作为本次唯一执行目标。
2. 读取最近的提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若有，将其视为当前任务的一部分或作为前置依赖写回 `TODO.md`。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证标准，以及必要时参考 `PLAN.md` 的阶段背景。
4. 检查实现现状，仅聚焦当前任务直接相关的代码与测试，避免开放式问题扫描。
5. 若可直接完成：实施代码修改，补充或更新测试，并运行任务要求的验证命令。
6. 若遇到阻塞当前任务的真实缺口或规格不匹配：先修复；若本次无法直接修复，则在 `TODO.md` 中插入最小必要前置任务，保持当前任务未完成，并停止继续推进。
7. 任务完成后：
   - 将该任务标题改为 `[DONE]`。
   - 更新 `TODO.md` 中的完成记录。
   - 仅当阶段计划确实变化时更新 `PLAN.md`。
   - 提交本次变更，提交信息使用任务号前缀。
8. 完成一个任务后立即停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 并识别当前应执行任务。
- 已识别第一个未完成任务为 `P7-T01A`：收口剩余 object/top-level init compiler-private function/global 命名到 `PrivateSymbolMangler`。
- 最近提交为 `[P7-T01R] Add init naming prerequisite`，与当前任务直接相关，说明该任务是最终 review 的前置补遗，而非可跳过问题。
- 下一步聚焦读取以下实现与测试入口：
  - `crates/scoopc/src/llvm/codegen/object_init.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  - 如有必要，再读取 `PrivateSymbolMangler` 的 authoritative API 定义与现有调用点。
- 关键澄清：`package.name` 与 `package.objectId.name` 这类源码 FQN 的确天然不同，不会相互冲突；本任务仍需保留 `role / namespace / declaration_kind`，因为同一 owner FQN 会派生多个 compiler-private 实体（init helper、bridge、guard、instance slot、backing global 等），不能只靠 FQN 文本命名。
- 已开始实施：
  - `object_init.rs` 已把 object init function/bridge/guard/instance/property global 迁到 `PrivateSymbolMangler`。
  - `mod.rs` 已把 top-level immutable init function/bridge/guard/value global 与 top-level var global 迁到 `PrivateSymbolMangler`。
  - `llvm/tests.rs` 已新增 production source inventory，并把定向测试收紧到新 private role family。
  - `tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop` 已从旧固定名字切到 hashed regex。
  - `crates/scoop_runtime/tests/once_guard_cross_dylib.rs` 已把示例 guard symbol 改成新的 representative private family。
- 下一步：运行 `cargo fmt`，然后按 `TODO.md` 指定顺序执行定向测试、全量 `scoopc` 测试与 `clippy`，若失败则修复后再回写 `TODO.md` 与提交。
- 验证已完成且通过：
  - `cargo fmt`
  - `cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
  - `cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  - `cargo test -p scoopc top_level_immutable_init_emits_explicit_root_frame_descriptor -- --nocapture`
  - `cargo test -p scoopc direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref -- --nocapture`
  - `cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver -- --nocapture`
  - `cargo test -p scoopc class_init_order_fixture_collects_class_init_println_call_bindings -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop`
  - `cargo test -p scoopc`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
  - `cargo test -p scoop_runtime once_guard_is_canonical_across_dylibs -- --nocapture`
- 已回写 `TODO.md`：`P7-T01A` 标记为 `[DONE]`，完成记录已补齐。
- 下一步：检查工作区 diff，按任务要求提交一次原子 commit，然后停止，不继续 `P7-T01R`。
