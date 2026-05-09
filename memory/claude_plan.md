# 执行计划

## 当前状态
- 已读取 `TODO.md`，第一项未完成任务为 `P8-T02R`：review legacy 主线删除结果。
- 已检查最新提交：`[P8-T02] Remove legacy effect lowering backend`，未见额外显式标注的未完 blocker。
- 本次只处理 `P8-T02R`，完成后立即停止。

## 执行步骤
1. 复核 `TODO-P8.md` 中 `P8-T02` / `P8-T02R` 的要求与验证项。
2. 检查 `crates/scoopc/src/effect/**`、`crates/scoopc/src/llvm/codegen/effect/**`、`crates/scoopc/src/llvm/emit.rs`、`crates/scoopc/src/llvm/mod.rs`、`crates/scoopc/src/llvm/frontend.rs`、`crates/scoopc/src/llvm/tests.rs`，确认 legacy effect/state-machine backend 与 code-shape-specific 入口是否真的删净，只剩中立 helper 或负向测试文字。
3. 重新运行 `P8-T02R` 要求的验证，至少包含 `P8-T02` 列出的定向测试/检查与额外搜索；若发现阻塞当前 review 的真实问题，则先修复或把最小前置任务写回 `TODO.md`。
4. 若 review 通过，则更新 `TODO-P8.md` 与 `TODO.md`，把 `P8-T02R` 标记为 `[DONE]` 并补充完成记录；若阶段计划未变，则不改 `PLAN.md`。
5. 提交本次改动并停止。

## 记录方式
- 在识别出具体任务后，补充更具体的执行说明。
- 在关键步骤完成或计划变化时，持续更新本文件。

## 当前检查重点
- 搜索命中中允许存在的内容：历史迁移注释、负向删除测试、非 effect 语义的普通 `legacy` 文本。
- 搜索命中中不允许存在的内容：effect/continuation 主实现里仍可执行的旧 lowering/backend 路径，或伪装成中立 helper 的旧入口。

## 进展记录
- 已复核 `crates/scoopc/src/effect/**` 与 `crates/scoopc/src/llvm/codegen/effect/**` 的目录形状：旧 `segments.rs` / `transform.rs` / `step_summary.rs` / `state_machine_bridge.rs` / `state_machine_emitter.rs` 确已删除，只剩共享 ordinary-callee 分析与当前 LLVM backend 代码。
- 复核过程中发现主实现仍残留少量误导性 `legacy` 命名/注释（不是完整旧 backend，但会干扰 P8-T02R 结论），已开始一并清理：包括 continuation payload helper 命名、effect call wrapper 变量名、callable carrier fallback 命名，以及若干注释/测试 helper 命名。
- 已完成 `cargo fmt`、`cargo check -p scoopc --features llvm`、`cargo test -p scoopc legacy_effect_backend_removed`、`cargo test -p scoopc single_effect_lowering_path`、`cargo clippy --all-targets -- -D warnings`。
- 已完成 P8-T02R 要求的搜索复核：旧 backend marker 仅剩 `crates/scoopc/src/llvm/tests.rs` 负向 inventory 测试字面量；泛化 `legacy` 搜索剩余命中已归类为负向删除测试、迁移说明、direct-HIR/dump 兼容注释或已删除语法诊断，并未发现 effect/continuation 主实现里的可执行旧路径。
- 已把 `P8-T02R` 在 `TODO-P8.md` 和 `TODO.md` 中标记为 `[DONE]`。
- 下一步：做提交前检查并创建本次任务提交，然后停止。
