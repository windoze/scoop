# Claude Plan

## Working Rules
- 先读取 `TODO.md`，只处理第一个未完成任务。
- 如果遇到阻塞当前任务的真实前置问题，只添加最小必要前置任务到 `TODO.md`，然后停止。
- 不做绕过式实现；实现后必须验证并更新记录。
- 在关键步骤完成或计划变化时，持续更新本文件。

## Initial Execution Plan
1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务。
2. 检查最近提交是否直接提到与该任务相关的未完成问题；若是，则将其视为当前任务范围的一部分或作为前置依赖记录到 `TODO.md`。
3. 阅读该任务涉及的代码、文档与测试，确认需求、依赖、现状与验证方式。
4. 实现该任务；如果发现阻塞当前任务的缺失功能或错误，先按要求更新 `TODO.md` / `PLAN.md`（仅在阶段计划变化时）并停止。
5. 运行与该任务相关的测试与必要的质量检查，修复发现的问题直到通过，或确认存在必须先处理的新前置任务。
6. 更新 `TODO.md` 中该任务的完成状态与 completion record；仅在阶段计划变化时更新 `PLAN.md`。
7. 更新本文件记录已完成步骤、验证结果与任何计划调整。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## Progress Log
- 已创建初始执行计划，下一步读取 `TODO.md` 并识别首个未完成任务。
- 已读取 `TODO.md`，确认首个未完成任务是 `P2-T01`：关闭 `comptime_*` 与 top-level `val` 的 pre-MIR/MIR gap。
- 已核对最近一次提交为 `[P1-T02] Remove legacy MIR resume and dispatch fallbacks`；提交内容聚焦 P1 收尾，未显式声明与 `P2-T01` 直接相关的未完成补丁，因此继续按 `P2-T01` 本体推进。
- 下一步：阅读 `PIPELINE_GAPS.md` 与 `P2-T01` 涉及的实现入口，确认当前 placeholder / `Item::Todo` 的具体流入路径，以及任务是否被新的真实前置问题阻塞。
- 已完成实现入口勘察，确认当前剩余 gap 主要是两个残余构造点：
  - `crates/scoopc/src/hir/lower/stmt.rs` 在缺少 runtime comptime plan 时仍会构造 `StmtKind::Todo("comptime_*")`。
  - `crates/scoopc/src/mir/lower.rs` 在非 typed-contract 路径仍会把 top-level `val` 构造成 `Item::Todo { kind: "top-level val" }`。
- 已运行现有定向测试：`refactor_mir_item_graph_publishes_top_level_roots`、`refactor_mir_comptime_splice_class_literal_and_with_update_preclosure`、`refactor_mir_placeholder_inventory`，当前均通过；说明主路径能力已基本存在，剩余工作是把残余 placeholder 构造点与审计基线一起收口。
- 当前实施方案：
  1. HIR lowering 总是优先处理 `comptime block/if/for`；`if/for` 若缺少 runtime comptime plan，则记录明确 stage error，而不再产出 `StmtKind::Todo`。
  2. MIR lowering 不再把 top-level `val` 降成 `Item::Todo`；改为统一依赖 initializer/extern root contract，并让 `MirLoweringFacts::from_lowered_hir(...)` 也携带这些 root contract，使 `lower_for_dump` / materialization 入口同样拥有 canonical root。
  3. 同步更新 HIR/MIR placeholder inventory、synthetic no-Todo 测试、`PIPELINE_GAPS.md` 和 `TODO.md` 完成记录，然后跑验证并提交。
- 已完成代码修改：
  - `hir/lower/stmt.rs` 已去掉 `comptime_*` 的 HIR `Todo` 构造点，缺计划时改为 stage error。
  - `mir/lower.rs` 已去掉 top-level `val -> Item::Todo` 分支，并让 `from_lowered_hir(...)` 发布 initializer / extern root contract。
  - HIR/MIR placeholder inventory、synthetic item-Todo 测试、`PIPELINE_GAPS.md`、`TODO.md` 已同步回写。
- 已完成验证，结果全部通过：
  - `cargo test -p scoopc refactor_hir_placeholder_inventory`
  - `cargo test -p scoopc refactor_mir_placeholder_inventory`
  - `cargo test -p scoopc dump_mir_emits_top_level_initializer_and_extern_roots`
  - `cargo test -p scoopc refactor_mir_item_graph_publishes_top_level_roots`
  - `cargo test -p scoopc refactor_mir_comptime_splice_class_literal_and_with_update_preclosure`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/top_level_roots.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/top_level_val_with_type_ok.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步：检查 git 变更摘要，创建一次 `[P2-T01] ...` 提交，然后停止。
