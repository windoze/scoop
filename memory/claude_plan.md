# 执行计划

## 范围

- 只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务。
- 不做开放式历史问题扫描；只处理与当前任务、当前验证失败或明确阻塞相关的问题。
- 若遇到必须先修复的具体前置问题，则按要求更新 `TODO.md`，提交后停止。

## 步骤

1. 读取 `TODO.md`，确定第一个未完成任务及其验证要求。
2. 查看必要上下文，包括相关代码、测试、最近提交与当前工作区状态，避免覆盖他人改动。
3. 按任务要求实现最小正确变更。
4. 运行相关验证；若观察到未排期的失败，修复或把最小前置任务加入 `TODO.md`。
5. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]` 并填写完成记录；必要时只在阶段计划变化时更新 `PLAN.md`。
6. 运行最终相关验证，检查工作区差异。
7. 用清晰任务编号提交本次变更，然后停止。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务为 `P7-T04-b-4R：Review codegen MonoTypeId 全面切换`。
- 最近提交为 `cbb8530a [P7-T04-b-4] Migrate codegen types to MonoTypeId`，与当前 review 直接相关。
- 已完成第一轮静态审查：`cg_ty_of` 本体已是 `MonoTypeId -> CgTy`，`expect_cg_ty_of` / `monomorph miss` / 直接 `MonoTypeId(` 构造未发现残留。
- 发现需在 review 内修复的缺口：`type_layout` 仍以 raw `TypeId` 做内部布局递归并保留 generic fallback；部分 effect-lowered source `TypeId` 被送进主 codegen `TypeStore` lowering；published callable signature 未显式携带其 source `TypeStore` owner。
- 已实施修复：`type_layout` 收紧为 `MonoTypeId`；effect-lowered source lowering 改走 owner-aware `cg_ty_of_mir_type`；published callable signature 现在携带 `TypeStore` owner，桥接失败不再返回 raw foreign `TypeId`。
- 已通过验证：`cargo fmt`；`cargo test -p scoopc llvm::codegen`；`cargo test -p scoopc_types`；`cargo test -p scoopc --no-default-features hir`；`cargo test -p scoopc --no-default-features mir`；`cargo test -p scoopc --no-default-features llvm::codegen`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`。
- 已通过最终检查：`cargo clippy --all-targets -- -D warnings`；`git diff --check`；关键残留搜索。
- 已更新 `TODO.md` / `TODO-6.md`，将 `P7-T04-b-4R` 标记为 `[DONE]` 并填写完成记录。
- 下一步检查 git diff/status/log，提交本任务变更后停止。
