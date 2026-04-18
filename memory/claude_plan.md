# 执行计划

说明：我会在这个文件中持续维护“可公开的执行计划、进度、结论与变更记录”。出于安全原因，这里不写逐字的完整内部思维链路，但会完整记录可审计的步骤、判断依据与后续动作。

## 初始计划

1. 检查最新一次 Git 提交的提交信息与改动，确认其中是否提到尚未修复的问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并确认是否需要拆分为更小子任务。
3. 如发现前置缺陷或规范不匹配：
   - 先修复该问题，或
   - 若当前无法直接修复，则按要求更新 `TODO.md` / `PLAN.md` 的依赖顺序后停止。
4. 对当前应执行的第一个任务进行实现。
5. 运行相关测试与校验，至少包括与改动相关的测试；若适用，运行格式化、`cargo test`、`cargo clippy --all-targets -- -D warnings` 等检查。
6. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成状态与必要说明。
7. 生成一次清晰的 Git 提交，然后停止，不继续做下一个任务。

## 进度记录

- 已创建本计划文件，待开始仓库检查。
- 已检查最新提交 `c97190b [T3017] Recover effect run-pass baseline`；提交正文未额外声明新的待修复遗留 issue。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T3017R`：复审统一 effect 主线是否已成为稳定 passing baseline。

## 当前任务：T3017R

### 任务理解

- 这不是新增功能，而是一次生产代码与测试基线形态复审。
- 需要确认：
  1. `tests/fixtures/run-pass/**` 中与 effect 统一主线相关的历史临时 xfail 已真正回收；
  2. 生产代码没有为了“只让 fixture 过”而引入 shape-based / flag-based / effect-only fallback；
  3. 当前绿色基线来自真实生产实现，而不是 test-only workaround。

### 当前执行计划

1. 复核 `T3017` / `T3017R` 任务定义与最近 effect 主线收口记录。
2. 检查 run-pass 基线形态：
   - 搜索 `tests/fixtures/run-pass` 中残留的 `EXPECT: fail`、`T3006`、`xfail` 注释；
   - 确认剩余失败用例是否都属于真实失败语义或已在后续任务中明确跟踪。
3. 审查生产代码：
   - 定向检查 `crates/scoopc/src/llvm/codegen/effect/mod.rs`
   - 定向检查 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - 检索是否残留 shape-based / flag-based / fixture-only fallback 入口。
4. 运行验证命令：
   - `cargo run -p scoop --features llvm -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若审查发现问题，直接修复并重新验证；若未发现问题，则更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 审查结果

- `tests/fixtures/run-pass` 中已无 `T3006: 暂时标记为 fail` 临时注释残留。
- `run-pass` 目录中 `EXPECT: fail` 仅剩 6 条：
  1. `effect_resume_double_resume_exit.scoop`
  2. `exit_code_mismatch.scoop`
  3. `stderr_mismatch_distinguishable.scoop`
  4. `timeout_should_fail.scoop`
  5. `gc_continuation_multi_thread_concurrent_alloc_resume.scoop`（已转记 `T3304`）
  6. `not_null_assert_basic.scoop`（已转记 `T3406`）
- 已检索 effect 生产代码，未发现旧的 shape-based / flag-based 主路径回流：
  - `emit_effect_unwind_if_active`
  - `raise_target_stack`
  - `scan_for_callee_suspend`
  - `codegen_top_level_fun_suspendable`
  - `codegen_closure_fun_body_suspendable`
  - `CalleeSuspendResumeMode`
- 已确认 `handle_propagate` 与 ordinary outward propagation 仍共享统一合同，没有为 fixture 通过而加入 test-only fallback。

## 验证结果

- `cargo run -p scoop --features llvm -- test`：通过，结果 `fixtures: ok (992)`。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- runner 过程中出现的 `WARN` 输出来自部分 fixture 触发的语义诊断日志，不属于 Rust 编译 warning 或 clippy warning。

## 收口动作

1. 将 `TODO.md` 中 `T3017R` 标记为完成，并写入审查结论。
2. 将 `PLAN.md` 更新为：`T30` 已阶段性完成，下一项转入 `T3103`。
3. 提交本轮变更后停止，不继续执行后续任务。
