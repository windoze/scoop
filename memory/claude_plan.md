## 当前执行计划

1. 已完成：读取 `TODO.md` 并定位当前任务为 `P7-T01R`（第一个未标记 `[DONE]` 的条目）。
2. 检查最新提交信息，确认是否存在与 `P7-T01R` 直接相关且明确未完成的问题；若存在，将其纳入当前 review 范围或按要求补充为前置任务。
3. 阅读 `P7-T01R` 引用的 `PLAN.md` §5/§6 与 `STABLE_ID.md` §10/§11/§12，整理最终签收需要逐项覆盖的结论。
4. 复查仓库当前状态，并按 `P7-T01R` 要求重跑 `P7-T01` 的关键审计与回归；若 review 暴露新的真实 blocker，则先修复或在 `TODO.md` 插入最小前置任务并停止。
5. 若 review 结论成立，则更新 `memory/claude_plan.md`、把 `P7-T01R` 标记为 `[DONE]` 并补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 以 `P7-T01R` 对应信息提交 Git，然后停止，不进入下一任务。

## 约束与执行原则

- 只完成 `TODO.md` 中第一个未完成任务。
- 不做开放式历史问题排查；仅处理当前任务直接相关或阻塞当前任务的问题。
- 如果存在规格不匹配、缺失能力或实现边界缺口，不使用 workaround；要么直接修复，要么在 `TODO.md` 中加入前置任务并停止。
- 执行过程中若计划改变或关键步骤完成，及时更新本文件。

## 当前进展

- 已确认当前任务：`P7-T01R：Review 全量收口结果，确认 stable-id 方案已闭合且未带来功能漂移`。
- 已核对最新提交 `[P7-T01A] Mangle init private symbols`，未发现提交正文中声明的未完成直接后续项；当前 review 范围维持 `P7-T01R` 本身。
- 已复核 `PLAN.md` §5/§6 与 `STABLE_ID.md` §10/§11/§12，确认最终签收需要覆盖 8 条完成标准、grep 审计与“无功能漂移”矩阵。
- 已完成第一轮定向验证：
  - `checkout_root` / `distinct_virtual_cones` 通过。
  - `stable_id_audit_grep_inventory_scans_repo_roots` 通过，且 `__schema[0-9]+`、`__k[0-9]+`、`t[0-9]+__` 为 0 命中；旧 `scoop.lambda*` 命中仅剩测试清单。
  - `external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke`、`stable_id_source_inventory`、init/object/class 相关 LLVM 回归、`once_guard_is_canonical_across_dylibs` 全部通过。
- 已完成 `P7-T01` 的全量行为/fixture/spec/lint 矩阵：
  - `cargo test -p scoopc`
  - `cargo test -p scoop_runtime`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以及 review 需要的 build/run-pass 定向回归
- review 过程中发现新的真实 blocker，因此不能签收 `P7-T01R`：
  - `crates/scoopc/src/llvm/codegen/mod.rs`、`gc.rs`、`mir_body.rs`、`ty.rs`、`composite_transport.rs` 仍有 active production private LLVM type/global naming 直接由 `sanitize_llvm_ident(...)`、`TypeStore::display()` 或 raw `TypeId` / `source_ty.as_u32()` 控制。
  - 这与 `STABLE_ID.md` §5.1 第 5/7 条及 `PLAN.md` §6 第 1/7 条冲突，因此已在 `TODO.md` 新增前置任务 `P7-T01B`，并把 `P7-T01R` 依赖更新为 `P7-T01A + P7-T01B`。
- 下一步：整理本次 blocker 发现、提交 `TODO.md` 与 `memory/claude_plan.md`，然后停止，等待下一次调用执行 `P7-T01B`。
