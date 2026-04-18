# 执行计划

## 说明

按要求先记录执行思路与计划。这里记录的是可审计的高层决策、执行步骤与进度，不包含冗长的内部推理细节。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现被更早的缺陷或规格不匹配阻塞，则先把阻塞项调整到正确优先级，并在本轮只处理最前面的那个事项。

## 初始步骤

1. 检查最新一次提交，确认提交说明或变更中是否提到现存问题；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划与任务依赖。
4. 如首个未完成任务过大，则把它拆分为更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的第一个子任务。
5. 实现当前目标任务。
6. 运行相关测试，并补充必要测试，确保实现正确。
7. 运行格式化、检查与质量命令，至少覆盖与本次修改相关的范围；若仓库要求允许且时间可控，执行全量检查。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态和依赖变化。
9. 提交 Git commit，然后停止，不继续处理下一个任务。

## 进度记录

- `2026-04-18`：初始化计划文件，尚未开始仓库检查。
- `2026-04-18`：已检查最新提交、`TODO.md`、`PLAN.md`。最新提交未附带额外“待修 issue”说明；当前首个未完成任务确认为 `T3016iR`，属于对 unified `SuspendCall` inactive helper IR 修复的复审任务。
- `2026-04-18`：下一步进入定向代码审查，重点查看 `state_machine_plan.rs`、`control_flow.rs`、`state_machine_emitter.rs` 中与 `T3016i` 相关的生产路径，并检索是否存在 helper/fixture/source-shape 特判回流。
- `2026-04-18`：复审过程中发现真实生产缺口：`when` arm 内 `return Ask.ask()` 的 ordinary callee/resumed-tail 会再次触发 LLVM verifier（`when_arm_*` / `when_merge` 在 terminator 后仍被追加指令）。决定在 `T3016iR` 内直接修复，而不是拆出新任务，因为它属于同一条 control-flow / resumed-tail 合同审查范围。
- `2026-04-18`：已完成代码修复：
  - `crates/scoopc/src/llvm/codegen/control_flow.rs`：`codegen_when_expr()` 不再给已终止的 arm 追加 merge branch；当所有 arm 都终止时，直接把 `when` 表达式结果收口为 `CgTy::Never`。同时 `codegen_block_as_return_value()` / `codegen_block_value_in_expected_context()` 在遇到 `Never` 时立即停止发射后续语句，避免继续往已终止 block 追加指令。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`：把 `when` 纳入 `expr_guarantees_control_flow_exit()`，确保 ordinary callee resumed tail 在“所有 arm 都返回/退出”时不再拼接 enclosing block 的 unreachable suffix。
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`：新增 IR 回归 `ordinary_callee_resume_site_drops_unreachable_suffix_after_when_all_arms_return`，锁定 `resume_site0` 在 `when` 场景下也会直接结束于返回路径。
- `2026-04-18`：已完成验证：
  - `cargo fmt --check`
  - `cargo test -p scoopc ordinary_callee_resume_site_drops_unreachable_suffix_after_ -- --nocapture`
  - 临时最小复现程序 `cargo run -p scoop --features llvm -- run <tmp>`，输出为 `helper_before / helper_suspend / 3`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- `2026-04-18`：已更新 `TODO.md` / `PLAN.md` / 本文件，`T3016iR` 已标记完成；当前只剩检查 worktree 并提交本轮 commit。
