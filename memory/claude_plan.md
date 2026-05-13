## 本次执行计划

1. 读取 `TODO.md`，确认当前首个未完成任务；初始识别为 `P7-T01R`。
2. 对照 `PLAN.md` §6 的 8 条完成标准，以及 `P7-T01` / `P7-T01A` / `P7-T01B` 的完成记录，整理本轮最终验收清单。
3. 复查 stable-id 审计入口与当前工作区状态，运行 `P7-T01R` 需要的最终审计 / 回归命令，确认没有新的 namespace、identity 或语义漂移问题。
4. 若复核通过，则在 `TODO.md` 中将 `P7-T01R` 标记为 `[DONE]` 并补全完成记录；若发现阻塞性缺口，则把最小必要前置任务写回 `TODO.md`，保持 `P7-T01R` 未完成，并让新前置任务成为新的首个未完成项。
5. 更新本文件记录关键结论，随后提交本次结果并停止。

## 进度记录

- 已读取 `TODO.md`，初始确认首个未完成任务为 `P7-T01R：Review 全量收口结果，确认 stable-id 方案已闭合且未带来功能漂移`。
- 已检查最近一次提交：`[P7-T01B] Mangle private metadata type globals`。提交信息未声明新的未完成缺口，因此进入最终 review / 验收分析。
- 已对照 `PLAN.md` §6 与 `P7-T01` / `P7-T01A` / `P7-T01B` 完成记录复核关键收口点，并对仓库执行了定向 grep / 代码阅读。
- 复核中确认新的阻塞性缺口：active RTTI / runtime-match identity 仍有路径让 `TypeStore::display()` / `sanitize_llvm_ident()` 承担 authoritative hash 输入，具体证据包括：
  - `crates/scoopc/src/rtti/mod.rs:314-315`：`type_rtti()` 直接 `stable_rtti_type_id(&self.types.display(ty).to_string())`。
  - `crates/scoopc/src/llvm/codegen/mod.rs:7505-7506`：interface runtime-match 直接 `stable_rtti_type_id(&self.types.display(target_ty).to_string())`。
  - `crates/scoopc/src/llvm/codegen/mod.rs:8861-8864`、`crates/scoopc/src/llvm/codegen/mir_body.rs:7748-7753, 7781-7788`、`crates/scoopc/src/llvm/codegen/gc.rs:1263-1319`：若干 derived type descriptor 仍用 display/sanitize 生成的 `canonical_name` 参与 `stable_rtti_type_id(...)`。
- 该缺口直接阻塞 `P7-T01R` 的“external surface 已脱离 pretty text 直接控制”与 `PLAN.md` §6 第 7/8 条签收，因此不能在本次 invocation 中把 `P7-T01R` 标记完成。
- 已在 `TODO.md` 新增最小必要前置任务 `P7-T01C：收口剩余 RTTI / runtime-match type_id 对 pretty text / sanitize 的依赖`，并将 `P7-T01R` 的依赖更新为包含 `P7-T01C`。
- 下一步：检查最终 diff，提交本次 blocker 记录与任务重排，然后停止；下一次 invocation 将从新的首个未完成任务 `P7-T01C` 开始。
