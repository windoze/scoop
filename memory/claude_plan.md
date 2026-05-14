## 当前执行计划

1. 读取 `TODO.md`，定位按顺序排列的首个未完成任务（仅标题带 `[DONE]` 才算完成）。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务前置条件，则按要求更新 `TODO.md`。
3. 阅读当前任务涉及的实现、测试与文档位置，确认约束、依赖与验收要求。
4. 实施当前任务所需的最小正确改动；如果遇到阻塞当前任务的真实缺陷或缺失能力，不做绕过，而是在 `TODO.md` 中增加最小前置任务并停止在该步。
5. 运行与当前任务直接相关的验证；至少覆盖任务要求的测试，并补充必要回归测试。若适用，再运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 等仓库级检查。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 若任务完成，则将对应任务标题改为 `[DONE]`，补全 `TODO.md` 中的完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
8. 检查工作区变更，按任务要求创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进展记录

- 已创建初始执行计划，下一步读取 `TODO.md` 识别首个未完成任务。
- 已确认首个未完成任务为 `P3-T02`：`收口 ctor/default-arg typed contract，删除 backend 补参/猜测`。
- 最新提交为 `[P3-T01] Gate raw MIR route-unsafe shapes before body emission`，未见直接声明 `P3-T02` 的未完成前置问题；继续按 `P3-T02` 本体实施。
- 下一步聚焦读取 `PLAN.md` / `PIPELINE_GAPS.md` 对应段落，以及 `mir/lower.rs`、`pipeline/mir_stage.rs`、`llvm/codegen/mir_body.rs` 中 ctor/default-arg contract 的当前实现与测试覆盖。
- 已确认当前任务的两个直接残留入口：
  - `crates/scoopc/src/llvm/codegen/class_ctor.rs` 仍允许在缺少完整 contract 时按 ctor span 缺失 / 参数个数回退选择 ctor，并在 `call_info=None` 时自行构造 positional 映射。
  - `crates/scoopc/src/llvm/codegen/mir_body.rs::codegen_mir_class_ctor_call(...)` 仍把 pass-MIR class ctor contract 压缩成 `ordered_param_count = args.len()`，没有显式继承 side table 发布的 ordered-args 长度。
- 计划中的实现收口：
  1. 删除 `class_ctor.rs` 中基于 arity 的 ctor 猜测与 `call_info=None` 的非 contract positional fallback，仅保留“无显式 ctor 且零参数”的隐式空 contract 情形。
  2. 让 pass-MIR class ctor codegen 继承 `CtorCallInfo.arg_mapping.len()` 作为 ordered-args contract，而不是现场从 `args.len()` 推导。
  3. 扩展 `mir_refactor/call_contracts.scoop` 与相关单测，锁定 class ctor named/default args 已在 upstream canonicalize 成完整 ordered args。
  4. 任务完成后回写 `PIPELINE_GAPS.md`、`codegen_gap_inventory.rs`、`TODO.md` 的 `P3-T02` 记录，并跑针对性测试、`clippy` 与任务要求的 fixture。 
- 已完成实现：
  - `crates/scoopc/src/llvm/codegen/class_ctor.rs` 不再按 arity 猜 ctor，也不再在缺少 `CtorCallInfo` 时构造 positional fallback；错误语义改成 selected/ordered args contract 失配。
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` 的 pass-MIR class ctor 路径改为继承 `CtorCallInfo.arg_mapping.len()` 作为 `ordered_param_count`。
  - 新增 HIR/MIR/LLVM 定向单测，分别锁定 ctor default args 的 contract 发布、MIR ordered args lowering、LLVM 最终构建。
  - 已回写 `PIPELINE_GAPS.md`、`codegen_gap_inventory.rs` 与 `pipeline_gap_audit.rs`，将 `§3.9`、`§3.10` 收口到 closed/re-scoped + contract guard 语义。
- 已完成验证：
  - `cargo test -p scoopc refactor_hir_ctor_contract_canonicalizes_default_args_to_ordered_slots`
  - `cargo test -p scoopc refactor_mir_call_contract`
  - `cargo test -p scoopc refactor_mir_ctor_default_args_lower_to_ordered_class_ctor`
  - `cargo test -p scoopc refactor_llvm_call_contract_lowering`
  - `cargo test -p scoopc refactor_llvm_ctor_default_arg_contract_lowering`
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc pipeline_gap_audit`
  - `cargo test -p scoopc llvm_tests`（按当前仓库过滤结果为 0 tests）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/call_contracts.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/default_param_call_site_fill_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 额外记录：`cargo test -p scoopc llvm::tests` 暴露了 8 个与当前任务无关的现有失败（closure/function-value/explicit-root-frame 相关）；这些失败不来自本次修改，且不构成 `P3-T02` 的前置阻塞，因此未改写 `TODO.md` 顺序。
