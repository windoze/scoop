## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务，并以其作为本次唯一执行目标。
2. 检查最近提交是否直接提到与该任务相关的未完成问题；如果存在且会阻塞该任务，则将其视为当前任务的一部分或在 `TODO.md` 中登记为前置任务。
3. 阅读任务说明、依赖、验证要求，以及相关代码与测试，确认需要修改的最小范围。
4. 直接实现该任务；如果遇到会阻塞任务完成的真实缺口或规格不匹配，则先修复该问题，或按要求把最小必要前置任务加入 `TODO.md`，并停止在该步骤。
5. 运行任务要求的验证命令，以及必要的构建、测试、格式化、lint；修复所有由本任务引入的问题，直到验证通过。
6. 更新 `memory/claude_plan.md` 记录关键进展和任何计划调整。
7. 在任务真正完成时，更新 `TODO.md`：给任务标题加上 `[DONE]`，并填写完成记录；仅当阶段计划本身变化时才更新 `PLAN.md`。
8. 检查工作区改动，按要求创建一次清晰的 git 提交，提交信息使用当前任务编号，并在提交后停止，不继续做下一个任务。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md` 并确认本次唯一目标为首个未完成任务 `CG-T07S0a21`：修复剩余 plain callable / ctor ABI 回归（top-level generic named args、cross-file ctor named/default、unsafe `FunPtr` aggregate return）。
- 已检查最近提交；最新提交 `[CG-T07S0a20]` 与当前任务不同，未发现需要作为 `CG-T07S0a21` 直接前置处理的最新提交未完成事项。

## 当前细化步骤

1. 复现 `CG-T07S0a21` 记录的三个失败：
   - `top_level_generic_named_args_basic.scoop`
   - `unsafe_funptr_aggregate_return_tuple.scoop`
   - `run_pass_cone/cross_file_ctor_named_default_basic`
2. 阅读相关 fixture、失败输出与 call/ctor/ABI lowering 代码，确认三个失败是否源于同一 authoritative contract 漂移；若出现新的真实前置阻塞，则按要求回写 `TODO.md`。
3. 以最小改动修复 authoritative call-site / ctor-site / indirect-call contract，不在 LLVM backend 现场猜语义。
4. 为修复点补或更新最小定向测试，并重跑任务要求的 build/test/scan 命令。
5. 更新 `FAILED_FIXTURES.md` 与 `TODO.md` 的完成记录；仅在阶段计划变化时更新 `PLAN.md`。
6. 检查工作区后提交一次 git commit，并停止。

## 当前发现

- 复现阶段发现三个目标 fixture 在当前工作区上均已通过，说明仓库中已存在与 `CG-T07S0a21` 对应的未提交修复。
- `git status --short` 显示以下相关未提交文件：
  - `crates/scoopc/src/hir/lower/expr.rs`
  - `crates/scoopc/src/mir/lower.rs`
  - `crates/scoopc/src/llvm/codegen/call/dispatch.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  - `runtime/c/scoop_test.c`
- 当前判断这是一次中途中断后留下的本任务工作；后续若验证全部通过，将按用户要求把这些未提交文件与文档更新一起提交。
- 已完成三组 fixture 的 build/test/scan 与两条新增 LLVM 回归单测；当前唯一额外问题是 `cargo clippy --all-targets -- -D warnings` 报出的 `needless_borrow`，正在做最小修正后重跑。
- `needless_borrow` 已修复，`cargo fmt` 与 `cargo clippy --all-targets -- -D warnings` 现已通过。
- 已开始文档收口：更新 `TODO.md` 将 `CG-T07S0a21` 标记为完成，并更新 `FAILED_FIXTURES.md` 删除这三个已修复 blocker。
