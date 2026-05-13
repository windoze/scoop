# Claude Plan

## 当前状态
- 已确认 `TODO.md` 中首个未完成任务为 `P3-T01：用 StableClosureKey 替换 closure private naming，并清理旧 alias 兼容层`。
- 最新提交为 `[P2-T02R] Review linkage containment`，提交信息未声明与 `P3-T01` 直接相关的未完成修复项。
- 工作区中已存在一批未提交改动，集中在 `stable_id.rs`、`llvm/codegen/{mod,closure,ordinary_callee,gc}.rs`、`effect_lowered/layout.rs`、`llvm/tests.rs` 等 `P3-T01` 目标文件，判断为本任务的半成品而非无关改动。
- 定向测试发现当前仍有一类真实漏口：materialized MIR / effect-lowered closure env type/type-desc 仍落在 `scoop.mir.lambda_env$...` / `__scoop_type_desc_mir_closure_env__...` 命名族，导致 `external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke` 失败。
- 当前处理方案：在 `llvm/codegen/mod.rs` 增加基于 HIR 词法遍历恢复 closure stable path 的共享 helper，并把 `mir_body.rs` / `effect_lowered/body.rs` 的 closure env 命名收口到 `StableClosureKey + PrivateSymbolMangler`；同时补一条 materialized MIR closure IR 测试与 source inventory 防回流检查。
- 已完成实现并通过验证：direct-HIR closure body/resume/env/type-desc 与 materialized-MIR closure body/env/type-desc 都已接入 `StableClosureKey + PrivateSymbolMangler`；effect-lowered closure env transport 同步复用同一 stable key，旧 direct-HIR carrier alias 兼容层也已清理。
- `TODO.md` 已将 `P3-T01` 标记为 `[DONE]` 并补全完成记录；下一步只剩创建本次任务提交并停止。
- 验证结果：
  - `cargo fmt`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc materialized_mir_closure_private_symbols_use_stable_hash_namespaces -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_ -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - 生产级 `crates/scoopc/src/llvm/codegen` grep：`scoop\.lambda\$[0-9]+` / `scoop\.lambda_resume\$[0-9]+` / `scoop\.lambda_env\$[0-9]+` / `scoop\.mir\.lambda_env\$` / `__scoop_type_desc_mir_closure_env__` 均为 0 命中。
- 本文件记录执行计划、关键进展、以及过程中如果出现的阻塞与计划调整。

## 初始执行计划
1. 读取 `TODO.md`，严格按标题是否带有 `[DONE]` 来识别首个未完成任务。
2. 读取与该任务直接相关的上下文（必要时包括 `PLAN.md`、相关源码、测试、最新提交信息），只聚焦当前任务，不做开放式问题扫荡。
3. 判断该任务是否可以直接完整实现；若存在会阻塞该任务的明确前置缺陷或缺失能力，则先在 `TODO.md` 中补充最小必要前置任务并停止。
4. 若无阻塞，则实现当前任务所要求的全部改动，保持实现与规范一致，不采用规避性方案。
5. 运行该任务要求的验证，以及必要的回归验证，包括相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`（若适用且可执行）、以及其他任务明确要求的检查。
6. 更新 `memory/claude_plan.md` 记录关键步骤和结果。
7. 在 `TODO.md` 中将已完成任务标题标记为 `[DONE]`，并更新完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
8. 检查工作区变更，按要求创建一次 git 提交，然后停止，不继续下一个任务。

## 约束
- 一次调用只完成 `TODO.md` 中当前顺序的一个任务。
- 不把带完成记录但标题未标 `[DONE]` 的任务视为已完成。
- 若发现阻塞当前任务的真实问题，必须先把它作为前置任务写入 `TODO.md`，而不是绕过。
- 不回退或覆盖我未创建的现有改动。
