## 本次执行计划

1. 先读取 `TODO.md`，确认第一个未完成任务（仅标题前缀带 `[DONE]` 的任务视为完成）。
2. 检查最近一次提交是否直接提到与该任务相关的未完成问题；如果是，则将其视为当前任务的一部分，或在 `TODO.md` 中登记为前置任务。
3. 阅读当前任务涉及的代码、测试、规格和依赖，确认实现边界与验证要求。
4. 直接完成当前任务；如果存在阻塞当前任务且无法绕过的真实缺陷或缺失能力，则先以最小必要粒度在 `TODO.md` 中加入前置任务并停止。
5. 运行与当前任务相关的验证；至少覆盖任务要求的测试，并补充必要回归验证。若任务落地，则尽量执行 `cargo fmt`、相关测试，以及 `cargo clippy --all-targets -- -D warnings`（如果范围或耗时异常，再根据实际情况说明）。
6. 更新 `memory/claude_plan.md` 记录关键进展；如任务完成，更新 `TODO.md`，将对应任务标题标记为 `[DONE]` 并填写完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 按仓库约定创建一次 git 提交，提交信息包含当前任务号，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可公开的执行计划与进度摘要，不包含内部推理细节。
- 若执行过程中发现阻塞项，会及时在本文件和 `TODO.md` 中补充说明。

## 进展更新

- 已读取 `TODO.md` 并确认当前首个未完成任务为 `P2-T02`：把 compiler-private helper 从 external namespace 收回 `internal/private`。
- 最近一次提交为 `[P2-T01] Classify LLVM declaration surfaces`，它对应当前任务的前置项，提交信息未额外暴露需先插入的新 blocker。
- 下一步：检查 `llvm` declaration/linkage helper、`object_init` / `mod.rs` / `closure` / `effect_lowered` 中仍保留 external 的 compiler-private helper，以及现有 object/symbol 审计测试，确认最小正确改动面。
- 已完成代码面收口：`object_init`、top-level immutable init、callee resume entry、materialized MIR closure/plain helper、closure body、effect helper/trampoline/thread-resume thunk 等 compiler-private helper 统一改为显式 `Linkage::Internal`；`effect_lowered` 中残留的裸 `add_function(..., None)` helper 声明路径也已改走统一 declaration helper。
- 已同步调整 `llvm/tests.rs`：helper smoke 测试改为检查 closure/effect/hidden-init helper 不再出现在 object external symbol 集，并额外用 IR 断言这些 helper 仍存在且使用 `internal/private` linkage。
- 已完成验证：`cargo fmt`、`cargo test -p scoopc function_declaration_ -- --nocapture`、`cargo test -p scoopc external_symbol -- --nocapture`、`cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings` 全部通过；额外 `rg -n "add_function\(" crates/scoopc/src/llvm/codegen` 只剩统一 declaration helper 内部的 `Some(linkage)` 调用。
- 已更新 `TODO.md`：将 `P2-T02` 标记为 `[DONE]` 并补全改动范围、核心决策、验证结果与闭合说明。
- 下一步：检查工作树差异，按任务号创建 git 提交，然后停止。
