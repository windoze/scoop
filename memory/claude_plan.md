## 当前执行计划

说明：按要求先记录执行计划与进度。这里记录的是可审计的执行步骤、假设、发现与变更，不包含内部推理细节。

### 初始计划

1. 读取 `TODO.md`，识别首个标题未带 `[DONE]` 的任务；不做开放式问题排查。
2. 查看最近提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务的一部分或作为前置任务写入 `TODO.md`。
3. 阅读该任务及其依赖中明确要求的相关代码、测试、文档位置，建立最小必要上下文。
4. 实现该任务要求；若遇到阻塞当前任务且不能以规范方式完成的缺陷或缺失能力，则在 `TODO.md` 中增加最小前置任务并停止继续后续任务。
5. 运行该任务要求的验证，以及受影响范围内必要的测试、格式化、lint（包括 `cargo clippy --all-targets -- -D warnings`，若适用）。
6. 更新 `TODO.md`：仅当任务真正完成时，在任务标题前加 `[DONE]`，并补充 completion record；若只是发现阻塞，则保持任务未完成并写明新增前置任务与依赖。
7. 仅在阶段计划、依赖结构或完成标准变化时更新 `PLAN.md`。
8. 检查工作区改动，保留非本人改动；将当前任务相关改动与必要的任务记录更新一起提交到 Git。
9. 停止，不进入下一个任务。

### 进度记录

- 已完成：创建本文件并写入初始计划。
- 已完成：读取 `TODO.md`，确认首个未完成任务为 `P2-T03`（`P2-T02` 已标记 `[DONE]`）。
- 已完成：检查最近提交，`git log -1` 为 `[P2-T02] Reject unterminated and empty non-Unit returns before codegen`，提交信息未显式声明与 `P2-T03` 直接相关的未完成事项。
- 已完成：检查工作区；当前仅有本文件未提交，未发现其他残留改动需要一并纳入当前任务。
- 已完成：运行 `P2-T03` 关键验证，发现 `tests/fixtures/run-pass/generic_fun_recursion.scoop` 失败，构成当前任务的直接阻塞。
- 已完成：直接复现失败，`cargo run -p scoop -- build tests/fixtures/run-pass/generic_fun_recursion.scoop ...` 报 `typed HIR call contract` 错误：`sysroot/print.scoop` 中 `call expression missing typed call-site contract`。
- 已完成：定位根因到 `crates/scoopc/src/typecheck/expr/call.rs::try_infer_where_bound_method_call(...)`。该路径对 `TypeKind::Param` 的 where-bound member call（如 `T: ToString` 上的 `value.toString()`）只返回类型，不记录 `TopLevelFunCallBinding`、`CallArgBinding`、member resolution 或 monomorph request，导致 HIR stage 无法发布 typed call-site contract。
- 已完成：修复 where-bound member call contract 发布缺口；`TypeKind::Param` receiver 的 bound-interface 调用现在会同步记录 member resolution、top-level call binding、arg binding、monomorph request 与 required effects。
- 已完成：新增 HIR-stage 回归测试 `refactor_hir_call_contracts_publish_where_bound_member_dispatch`，锁定 `where T: ToString` 上 `value.toString()` 会发布 `TypedCallSiteContract::Interface`。
- 已完成：更新 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，把 `PIPELINE_GAPS §2.3` 改写为 non-blocking upstream impossible-state guard，并新增 inventory 单测固定该语义。
- 已完成：更新 `PIPELINE_GAPS.md`，将 `§2.3`、`§2.5`、`§2.7` 回写为 `Closed/Re-scoped`，并同步修正建议收口顺序说明。
- 已完成：更新 `TODO.md`，将 `P2-T03` 标记为 `[DONE]` 并补齐完成记录。
- 已完成：验证通过：
  - `cargo test -p scoopc refactor_materialized_mir`
  - `cargo test -p scoopc codegen_gap_inventory`
  - `cargo test -p scoopc refactor_hir_call_contracts_publish_where_bound_member_dispatch`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/generic_materialization.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/generic_fun_recursion.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/tostring_interface_basic.scoop`
  - `cargo clippy --all-targets -- -D warnings`
- 说明：一次并行验证中的 `generic_fun_recursion.scoop` 曾因并行运行超时；随后单独串行复跑已快速通过，未复现真实挂死。
- 细化计划（针对 `P2-T03`）：
  1. 阅读 `P2-T03` 涉及的实现与测试入口：`mir/materialize.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/mir_body.rs`、`llvm/codegen/effect_lowered/value.rs`、相关测试与 `PIPELINE_GAPS.md` §2.3/§2.5/§2.7。
  2. 识别当前 materialized MIR 仍可能保留的三类问题：`MaterializedTodo`、missing root / missing generic template、concrete path `TypeKind::Param` 漏到 codegen。
  3. 以最小改动收紧 handoff contract：
     - materializer 对 missing template/root 直接报 source-level hard error；
     - 在 materialization 阶段消除 concrete path `TypeKind::Param` 漏出；
     - 下游 codegen guard 保留，但改成 impossible-state / compiler bug sentinel 语义。
  4. 补或收紧测试，覆盖 materialized root index / instance key 查询，以及 negative cases。
  5. 先修复当前阻塞：为 where-bound member call 补齐与普通 member dispatch 等价的 typed call contract 发布，确保 `generic_fun_recursion.scoop` 能通过，并避免 generic/sysroot 调用再次在 HIR stage 漏 contract。
  6. 回写 `PIPELINE_GAPS.md` 与 `TODO.md` 完成记录；若实现过程中发现真正前置阻塞，则先在 `TODO.md` 中插入最小前置任务并停止。
  7. 运行任务要求测试、`cargo clippy --all-targets -- -D warnings`，再提交当前任务。
- 下一步：检查工作区差异并创建 `P2-T03` 提交，然后停止。
