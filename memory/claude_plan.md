# 执行计划

1. 阅读 `TODO.md`，按标题是否带 `[DONE]` 识别第一个未完成任务，并检查任务正文、依赖、验证要求和完成记录。
2. 查看最新提交信息，仅在其明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 基于当前任务定位相关代码、测试和规格说明，避免开放式历史问题排查。
4. 以最小正确改动完成当前任务；如果发现阻塞当前任务的规格不匹配或缺失能力，先在 `TODO.md` 插入最小必要前置任务并停止。
5. 运行当前任务要求的验证以及必要的回归测试；若失败，修复后重测。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，补充完成记录；仅当阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区变更，提交本次任务相关改动，并在提交后停止，不继续下一个任务。

## 当前状态

- 已识别第一个未完成任务：`HIR-T00：审计并冻结 refactor HIR placeholder inventory`。
- 最新提交 `365dd1ad Update plan` 未明确提到与 `HIR-T00` 直接相关的未完成问题。
- 已新增 `crates/scoopc/src/hir/lower/placeholder_inventory.rs` 测试模块，冻结 `src/hir/**` 中的 HIR placeholder 构造点、分类和处理任务。
- 该测试同时记录 refactor typed HIR stage 当前不携带 legacy `lower_for_dump` dump-only fallback。
- 已修复 no-default-features 测试编译中的既有 dead-code warning：`LateLoweredSurfaceResumeDispatchInventoryEntry::new` 仅在 `all(test, feature = "llvm")` 下编译。
- 已通过验证：`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`rg "ExprKind::Todo|StmtKind::Todo|Item::Todo" crates/scoopc/src/hir`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 已将 `TODO.md` 中 `HIR-T00` 标记为 `[DONE]` 并补充完成记录；下一步检查 git 变更并提交后停止。
