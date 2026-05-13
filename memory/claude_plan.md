## 执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 检查最近提交是否存在与该任务直接相关且明确未完成的问题；若有且构成前置依赖，则先在 `TODO.md` 中显式记录依赖关系。
3. 阅读当前任务涉及的代码、测试、规范与相关文件，确认实现边界与验收要求。
4. 直接实现当前任务；若遇到阻塞当前任务且无法规避的缺失/缺陷，则在 `TODO.md` 中添加最小必要前置任务，更新依赖顺序后停止。
5. 运行当前任务要求的验证，以及必要的回归测试、格式化、`cargo clippy --all-targets -- -D warnings` 等质量检查；若失败则继续修复直到通过，或按前述阻塞流程处理。
6. 完成后更新 `TODO.md`：将当前任务标题前缀改为 `[DONE]`，补全完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
7. 将本次关键进展同步回本文件，包括：已识别任务、实施步骤调整、验证结果、是否完成提交。
8. 按任务号创建一次 git 提交，并在提交后停止，不继续执行下一个任务。

## 进度记录

- 已创建初始执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `P1-T02`：删除 resume/dispatch legacy producer，并清空 active `LegacyOnly` 依赖。
- 最近提交为 `[P1-T01] Remove legacy MIR call and assign producers`；它与当前任务直接相邻，但未额外声明新的未完成前置问题。当前先按 `P1-T02` 既定范围检查相关代码、inventory、guard 与测试。
- 已确认当前没有新的前置 blocker：`lower_for_dump` / materialize 仍处于“typed call/assign + fallback resume/dispatch”的混合状态，但这正是 `P1-T02` 需要收尾的边界，而不是额外拆分出的新任务。
- 当前实施策略：
  - 在不把 dump/materialize 整体切成 `RefactorTyped` 的前提下，把 resume 站点补接到现有 typed contract。
  - 删除 dispatch/resume legacy producer，不再通过 callee 形状恢复语义；typed contract 缺失时改为 impossible-state panic，而不是 `Todo(...)`。
  - 从 MIR/HIR placeholder inventory、preflight 禁词、verifier/materializer synthetic tests 中移除 `LegacyOnly` 与旧 legacy reason 绑定，改用 contract-neutral synthetic reason 覆盖 no-Todo guard。
  - 完成后回写 `PIPELINE_GAPS.md` 与 `TODO.md`，再执行验证与提交。
- 已完成的实现步骤：
  - `crates/scoopc/src/mir/lower.rs`：删除 resume/dispatch legacy producer；resume 改为只消费 typed contract；dispatch 不再保留 legacy owner/member 猜测路径；缺失 typed contract 时转成 impossible-state panic。
  - `crates/scoopc/src/mir/placeholder_inventory.rs`、`crates/scoopc/src/hir/lower/placeholder_inventory.rs`：移除 `LegacyOnly` disposition 与相关断言/条目。
  - `crates/scoopc/src/pipeline/hir_preflight.rs`、`crates/scoopc/src/pipeline/mir_stage.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs`、`crates/scoopc/src/pipeline/hir_stage.rs`、`crates/scoopc/src/mir/lower.rs` 测试区：移除对旧 legacy reason 的 active guard / whitelist / synthetic test 绑定，改成 contract-neutral no-Todo 断言或 synthetic reason。
  - `crates/scoopc/src/pipeline_gap_audit.rs`：将 `LegacyOnly` 命中与 legacy reason 命中基线收紧为 0。
  - `PIPELINE_GAPS.md`：将 `§1.8`、`§1.9` 回写为 `Closed/Re-scoped`，并同步更新 `§1.6`、`§1.7` 的 residual 描述。
- 已完成验证：
  - `cargo test -p scoopc refactor_hir_placeholder_inventory`
  - `cargo test -p scoopc refactor_mir_placeholder_inventory`
  - `cargo test -p scoopc pipeline_gap_audit`
  - `cargo test -p scoopc refactor_mir_call_contract`
  - `cargo test -p scoopc refactor_materialized_mir`
  - `cargo test -p scoopc materialized_pass_view_non_generic_dispatch_and_resume_roots_are_published`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
  - `cargo clippy --all-targets -- -D warnings`
  - `rg 'LegacyOnly|assign lhs missing local|assign lhs lowering pending|call callee lowering pending|ctor call lowering pending|sizeOf intrinsic requires value or type arg|nameOf intrinsic requires type arg|resume lowering requires canonical callee shape|dispatch callee lowering pending' crates/scoopc/src crates/scoop/src tests/fixtures`：0 命中。
- 已回写 `TODO.md`：`P1-T02` 已标记为 `[DONE]`，完成记录已补齐。
- 下一步：检查 git 工作树、确认提交范围，然后创建 `[P1-T02] ...` 提交并停止。
