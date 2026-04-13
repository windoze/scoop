# 执行计划与进度记录

## 说明

应要求先写入本文件，再开始执行任何命令。这里记录的是可审计的执行计划、关键决策和进度更新，不包含不可审计的内部推理原文。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息或变更中是否提到已知遗留问题。
2. 若最新提交暴露了需先修复的既有问题，优先修复这些问题，并补充测试。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 评估该任务的范围与依赖：
   - 如果任务足够明确且可在本轮完整完成，则直接实现。
   - 如果任务过大或存在前置缺口，则更新 `PLAN.md` 与 `TODO.md`，将其拆成更小的子任务，并只执行新的第一个子任务。
5. 实现当前应执行的首个任务。
6. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响面较广，补充运行更完整的测试集。
   - 按要求关注无警告构建与 `cargo clippy --all-targets -- -D warnings`。
7. 更新文档与计划：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时调整其顺序与依赖。
   - 在 `PLAN.md` 中同步当前状态。
   - 按需要更新本文件，记录关键步骤完成情况与计划变化。
8. 使用清晰的提交信息提交本轮改动。
9. 停止，不继续执行下一个任务。

## 进度

- 已完成：创建本计划文件。
- 已完成：检查最新提交 `366136afa5a69a78bb1a61fe1e47a6707141267d`。提交标题为 `[T2003r3b3] Route multi nonresuming handles through unified emitter`，未在提交信息中显式声明需要先修复的遗留问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，定位首个未完成任务为 `T2003r3c`。
- 任务评估：`T2003r3c` 当前可在本轮完成，不再拆分。原因是 single immediate-resume / single escape-continuation 的 leaf lowering 已存在，本轮主要缺口是把 `codegen_handle_expr` 的主入口切到统一 entrypoint，并补上 contract 校验与定向回归。
- 当前执行计划：
  1. 在 effect codegen 中新增 unified single-resuming entrypoint 分类与校验逻辑。
  2. 让 `codegen_handle_expr` 先走 unified single-resuming 入口，再分发到 immediate/escape leaf helper。
  3. 新增或更新定向单测，覆盖 single immediate / single escape 的 representative samples。
  4. 运行 `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`、代表性 LLVM fixture，以及 `cargo clippy --workspace --all-targets -- -D warnings`。
  5. 若验证通过，更新 `TODO.md` / `PLAN.md` / 本文件并提交。
- 已完成：实现 `UnifiedSingleResumingEntrypoint`，并让 `codegen_handle_expr` 通过 `codegen_handle_expr_unified_single_resuming(...)` 统一接管 `SingleImmediateResume` / `SingleEscapeContinuation` 主选路。
- 已完成：补 single immediate / escape 的 plan contract 校验，以及基于 unified plan suspend-site 形态的 zero-match/no-suspend 顺序回退。
- 已完成：新增定向单测：
  - `unified_single_resuming_entrypoint_marks_single_immediate_resume_while_nested_handle_sample`
  - `unified_single_resuming_entrypoint_marks_single_escape_direct_if_nested_handle_sample`
  - `unified_single_resuming_entrypoint_marks_single_escape_indirect_nested_handle_sample`
- 已完成：验证通过的命令：
  - `cargo test -p scoopc unified_single_resuming_entrypoint_ -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_if_branch.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop`
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 待执行：检查工作区差异，完成提交，然后停止。
