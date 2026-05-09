# 本次执行计划

说明：按安全约束，这里记录可审计的执行计划与进度，不记录私有推理细节。

1. 读取 `TODO.md`，识别第一个标题未加 `[DONE]` 的任务，严格以其顺序和正文为准。
2. 检查最近一次提交信息，判断是否存在与该任务直接相关且明确标注未完成的问题；若存在，则将其视为当前任务范围的一部分，或在 `TODO.md` 中补充为前置依赖。
3. 阅读当前任务相关的 `TODO.md` 条目、必要的 `PLAN.md` 段落，以及实现所需的最小代码范围，确认约束、依赖、验证要求与是否已有未提交工作需要一并处理。
4. 如当前任务可直接完成：
   - 在最小必要范围内实现修改；
   - 补充或调整测试；
   - 运行任务要求的验证以及相关回归检查；
   - 修复发现的问题，直到当前任务满足要求。
5. 如遇到阻塞当前任务的真实前置缺陷或缺失特性：
   - 不做规避性实现；
   - 在 `TODO.md` 中以最小粒度新增前置任务并调整顺序/依赖；
   - 仅在阶段计划受影响时更新 `PLAN.md`；
   - 保持当前任务未完成，然后提交这些任务编排更新并停止。
6. 若当前任务完成：
   - 在 `TODO.md` 中将该任务标题显式改为 `[DONE]`；
   - 更新完成记录，写明实现与验证结果；
   - 仅在阶段计划变化时更新 `PLAN.md`；
   - 视需要更新 `README.md` 或相关内联文档（仅在本任务确有必要时）。
7. 查看工作区状态，确保不回退他人修改；若本次是恢复未完成任务且存在相关未提交变更，则按用户要求在完成时一并提交。
8. 提交本次修改，提交信息以当前任务编号为前缀，准确描述完成内容或新增前置依赖。
9. 停止，不继续下一个任务。

## 进度记录

- 已写入初始执行计划。
- 已读取 `TODO.md`，识别首个未完成任务为 `P8-T03ab`：消除 object/top-level hidden-init LLVM helper 对 legacy `effect_outcome` / handler-stack swap 的隐藏依赖。
- 已检查最近一次提交信息：`[P8-T03ab] Track hidden-init blocker after LLVM cleanup`，与当前任务直接相关，说明该 blocker 已被正式前插并处于待修复状态。
- 已读取 `TODO-P8.md` 中 `P8-T03ab` / `P8-T04` 条目，确认当前任务要求：
  - 默认单文件 refactor helper 不得再显式调用 `scoop_effect_outcome_consume_current`、`scoop_effect_handler_stack_swap_top` 等 legacy ordinary-effect transport；
  - object/top-level hidden-init outward case 仍必须走 authoritative `Step` / continuation contract；
  - 至少验证两条指定 LLVM 单测，并建议补跑 raw-MIR 对应回归。
- 下一步：
  1. 运行两条指定 LLVM 单测，复现当前 blocker；
  2. 阅读 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs`、`crates/scoopc/src/llvm/codegen/object_init.rs` 及相关 helper；
  3. 最小修改 hidden-init boundary lowering，去掉默认 helper 对 legacy outcome/handler-stack shim 的依赖；
  4. 重新运行指定测试与补充回归；
  5. 更新 `TODO-P8.md`、`TODO.md`（若需）与本文件，最后提交并停止。

- 已复现 blocker：默认 helper `__scoop_refactor_direct_invoke__a_helper` 在 object/top-level hidden-init 上仍直接出现 `scoop_effect_handler_stack_swap_top` 与 `scoop_effect_outcome_consume_current`。
- 已完成实现：
  - 在 `crates/scoopc/src/llvm/codegen/object_init.rs` / `crates/scoopc/src/llvm/codegen/mod.rs` 增加内部 hidden-init bridge helper，收口 object init / top-level immutable init 的 explicit outcome capture；
  - 在 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 改写 class-ctor hidden-init 分支：默认 helper 改为调用 bridge 并在 helper 侧通过 `refactor_step_tag` switch 做 complete/outward dispatch，不再直接安装 legacy outcome boundary；
  - 在 `crates/scoopc/src/llvm/tests.rs` 增加 object property init 默认 helper 回归，并加强 object value / object property / top-level immutable 三条断言，禁止 helper body 再出现 legacy outcome / handler-stack shim。
- 已完成验证：
  - `cargo test -p scoopc --lib llvm::tests::object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  - `cargo test -p scoopc --lib llvm::tests::object_property_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  - `cargo test -p scoopc --lib llvm::tests::top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary -- --exact`
  - `cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_object_value_init_access -- --exact`
  - `cargo test -p scoopc --lib llvm::tests::production_codegen_lowers_raw_mir_top_level_immutable_init_access -- --exact`
  - `cargo clippy --all-targets -- -D warnings`
- 验证过程中发现 `cargo clippy --all-targets -- -D warnings` 暴露两个与本任务无直接语义关系的现有 lint（`too_many_arguments` 与 `if_same_then_else` / `obfuscated_if_else`）。已采用仓库现有做法做最小一致性修复：为两个 internal helper 补 `#[allow(clippy::too_many_arguments)]`，并把 `crates/scoop/src/commands/build.rs` 的两个 Clippy 报告改写成等价的直白表达，以保持本次提交在 `-D warnings` 下可通过。
- 当前状态：已完成 `TODO-P8.md` / `TODO.md` 状态回写；下一步仅剩提交本轮改动并停止。
