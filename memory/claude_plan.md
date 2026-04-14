# 当前执行计划

## 约束说明

- 本文件记录可执行计划、关键决策、进度更新与阻塞项。
- 出于协作与安全边界考虑，这里不记录不可外露的内部推理，只记录足以审计执行过程的明确步骤。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与阶段目标。
4. 评估第一个未完成任务是否足够小且可在本轮完整交付。
5. 如果任务过大：
   - 将其拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，确保首个未完成项变成新的最小子任务；
   - 仅执行新的首个子任务。
6. 如果任务可执行：
   - 先阅读相关代码、测试、规范与实现边界；
   - 找出任何阻塞该任务的既有缺陷或规格不匹配；
   - 若发现阻塞项，先按要求把阻塞修复任务加入 `TODO.md`/`PLAN.md` 并调整顺序，然后停止在该轮范围内要求的合适位置。
7. 实现当前目标任务，保持改动最小且符合仓库既有结构。
8. 运行相关验证：
   - 至少运行与本任务直接相关的测试；
   - 如改动影响较广，补充运行更高层验证；
   - 尽量满足无警告要求，包括需要时运行 `cargo clippy --all-targets -- -D warnings`。
9. 更新文档与计划：
   - 在 `TODO.md` 中把当前完成项标记为已完成；
   - 在 `PLAN.md` 中记录完成情况、后续影响与必要调整；
   - 视执行过程更新本文件。
10. 检查工作区差异，确认只包含本轮应提交内容。
11. 提交 Git commit，提交信息应清晰描述本轮完成的任务。
12. 停止，不继续处理下一个任务。

## 进度

- 已创建本计划文件，下一步将检查最新提交和任务列表。
- 已检查最新提交 `af7f5ec303046dc85fd09b091d8f42f2a3ff43aa`：
  - 提交信息为 `[T2999R] Review dead-code retention boundaries`。
  - 提交信息未声明新的待修复既有问题，因此无需在进入 `TODO.md` 前插入额外修复任务。
- 已阅读 `TODO.md` 与 `PLAN.md`：
  - 首个未完成任务为 `T3001`：删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径。
  - 该任务当前边界清晰，可在本轮直接实现，不需要先拆分子任务。
- 已完成对相关生产代码的初步定位：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 仍包含 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable`，且顶层函数 / closure 入口仍在调用这些旧路径。
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 目前只保留统一 state-machine 骨架与少量 effect helper；其中部分 helper 仅被上述旧路径引用。
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 与 `runtime_symbols.rs` 中仍保留 callee-suspend TLS state 的 ABI 声明与符号常量。

## 本轮实现计划（T3001）

1. 从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除旧的 callee-suspend shape-based 扫描与两条 suspendable codegen 入口，并把顶层函数 / closure codegen 收口回常规路径。
2. 清理因为第 1 步失去引用的辅助类型与 helper：
   - `CalleeSuspendLocal`
   - `CalleeSuspendInfo`
   - effect 模块中仅服务于旧 callee-suspend 路径的 helper
   - runtime ABI / symbol 中仅服务于旧 callee-suspend 路径的声明
3. 运行定向检查，确认：
   - 生产代码中不再出现 `T3001` 指定的四个旧符号；
   - `cargo check -p scoopc` 通过且无 warning；
   - 如有必要，再运行 `cargo clippy --all-targets -- -D warnings` 验证零 warning 门槛没有回退。
4. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T3001` 已完成及当前状态。
5. 提交本轮改动并停止。

## 本轮结果

- 已从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除旧的 callee-suspend shape-based 主路径：
  - `CalleeSuspendResumeMode`
  - `scan_for_callee_suspend`
  - `codegen_top_level_fun_suspendable`
  - `codegen_closure_fun_body_suspendable`
- 已把顶层函数与 closure 的 codegen 收口回常规路径，不再按 `perform` 所在源码形状进入专用 suspendable lowering。
- 已清理与该旧路径绑定的辅助代码：
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中的旧 payload/state helper
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中的 callee-suspend TLS ABI 声明
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` 中对应符号常量
- 已完成验证：
  - `rg -n "CalleeSuspendResumeMode|scan_for_callee_suspend|codegen_top_level_fun_suspendable|codegen_closure_fun_body_suspendable|CalleeSuspendLocal|CalleeSuspendInfo|declare_runtime_callee_suspend_state_(get|set|clear)|SCOOP_CALLEE_SUSPEND_STATE_(GET|SET|CLEAR)" crates/scoopc/src/llvm/codegen -S`
    - 无命中
  - `cargo fmt --all`
  - `cargo check -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
- 下一步不是继续实现，而是按流程更新 `TODO.md` / `PLAN.md` 后提交本轮 `T3001` 并停止；下一轮首个未完成任务应为 `T3001R`。
