## 当前执行计划

1. 已确认首个未完成任务为 `CG-T07S0a7`：修复 `literal_ops_compare_direct_matrix_basic.scoop` 中 String 字面量 receiver 的 `compareTo` / `concat` 直接调用退化成 `CallKind::FunValue`。
2. 最近一次提交 `[CG-T07S0a6] Fix UInt8 array literal expected-type absorption` 已明确默认 full-suite 下一处 blocker 就是该 fixture；未发现需要先补录的其他前置任务，当前按 `TODO.md` 直接执行。
3. 先复现当前失败：按任务要求执行定向 build/test，必要时查看 MIR/HIR，确认 direct member call 在哪里丢失 authoritative call-site contract。
4. 阅读与 String 成员直接调用、member access resolution、direct call lowering、materialized MIR/main codegen 相关的实现与现有回归测试，定位为何字面量 receiver 会退化成 unresolved member access + `FunValue` callee。
5. 以最小改动修复 authoritative direct-call lowering 主线，确保 `compareTo` / `concat` 保持已解析直接调用形状，而不是靠 backend 猜 callee 或改 fixture 规避。
6. 补最小回归验证，至少覆盖任务要求的 build/test；若默认 full-suite 继续暴露下一个 blocker，则按 `TODO.md` 顺序新增最小 prerequisite。
7. 完成后更新 `TODO.md`：将 `CG-T07S0a7` 标记为 `[DONE]` 并补充完成记录；仅当阶段计划变化时才更新 `PLAN.md`。
8. 提交本次变更，然后停止。

## 约束提醒

- 只处理 `TODO.md` 中当前排序下的第一个未完成任务。
- 不以变通方式绕过语言/运行时/规范缺口。
- 若存在阻塞，新增最小前置任务并提交后停止。

## 当前进展

- 已从 `TODO.md` 确认本轮任务是 `CG-T07S0a7`。
- 已从最近提交与 `TODO.md` 完成记录确认：`CG-T07S0a6` 修复后，默认 full-suite 的下一处 blocker 正是 `literal_ops_compare_direct_matrix_basic.scoop`。
- 已复现失败：`cargo run -p scoop -- build tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop -o /tmp/literal_ops_compare_direct_matrix_basic` 仍在 refactor plain main codegen 前端准备阶段把 `"ab".compareTo("ac")` 降成 `CallKind::FunValue`，报 `unsupported main codegen node: refactor plain function-value callee type`。
- 已定位根因：`typecheck/expr/call.rs` 对 `String.concat` / `String.compareTo` 走 builtin 早返回，只给出返回类型，不发布 extension/top-level call binding；因此 HIR/MIR 无法把成员调用 canonicalize 成 direct call，最终退化成 unresolved `MemberAccess` + `FunValue` callee。
- 已收敛到更小的最终方案：不再依赖新增 sysroot 声明，而是在 `typecheck/expr/call.rs` 为 `String.concat` / `String.compareTo` 直接发布 synthetic `ExtensionFun` member resolution 与 receiver-prefixed call-arg binding；legacy/refactor LLVM direct-call path 消费 `scoop.core.concat` / `scoop.core.compareTo` runtime lowering，`effect_facts::builder` 将其归为 plain compiler intrinsic，避免 effect-step body 错发 DynamicFallback call boundary。
- 验证通过：前端回归单测、`literal_ops_compare_direct_matrix_basic.scoop` 的 build / fixture test、`effect_refactor_no_legacy_handler_stack_calls.scoop` 定向 build fixture，以及 `cargo clippy --all-targets -- -D warnings`。
- 默认 full-suite 已越过 `literal_ops_compare_direct_matrix_basic.scoop`，当前下一处真实 blocker 为 `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`；已在 `TODO.md` 中把本任务 `CG-T07S0a7` 标记为 `[DONE]`，并新增 prerequisite `CG-T07S0a8` 记录该 ABI tuple payload/source-component 缺口。
