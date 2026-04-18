# 本轮执行计划

## 说明

按要求先记录执行计划、检查步骤和后续进度更新点。这里保留可审计的决策摘要与执行方案，不记录冗长的原始思维展开。

## 初始步骤

1. 检查最新一次 Git 提交的提交信息与改动，确认是否提到了尚未修复的既有问题；若有，优先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 如首个未完成任务过大，则将其拆分为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
5. 实现本轮目标任务。
6. 运行与改动相关的格式化、测试、lint/检查命令，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
8. 提交 Git commit，然后停止，不继续做下一个任务。

## 进度记录模板

- [x] 已检查最新提交
- [x] 已定位首个未完成任务
- [x] 已确认是否需要拆分
- [x] 已完成实现
- [x] 已完成验证
- [x] 已更新计划与任务清单
- [ ] 已创建提交

## 当前识别结果

- 最新提交 `d57cbf2c95c88ac7a53463307af6211afe8040aa` 的提交信息与计划文件都明确指出了一个既有生产回归：`effect_handle_suspend_call_inactive_helper_basic.scoop` 在 LLVM verifier 阶段报 `Terminator found in the middle of a basic block! label %resume_site0`。
- `TODO.md` 中首个未完成任务为 `T3016i`：修复 unified `SuspendCall` inactive helper 路径再次生成非法 LLVM IR 的回归。
- 当前判断该任务范围明确，可直接执行；暂不需要再次拆分。

## 当前执行步骤

1. 复现 `T3016i` 的 verifier 失败，拿到最小报错上下文。
2. 检查 `SuspendCall` inactive helper 相关的 unified state-machine emitter / plan 逻辑，定位是哪条 resume-site/caller-tail 分支生成了重复 terminator。
3. 修复生产代码，并补足定向测试。
4. 运行格式化、定向测试、全量测试与 clippy。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 当前结果

- 已确认 `T3016i` 不需要再拆分；本轮直接完成该任务。
- 已定位根因：ordinary callee resumed tail 的 `build_resume_tail_block_from_stmt_slice()` 会在首个重建语句已经保证退出控制流时，继续拼接同层 sibling statements，导致 `%resume_site0` 在 `br label %return` 后仍生成 unreachable suffix，触发 LLVM verifier。
- 已在 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 中按统一 resume-path 合同截断这类 unreachable suffix。
- 已在 `crates/scoopc/src/llvm/codegen/control_flow.rs` 中修复 block expression dead path 的 typed dummy value 返回，避免 verifier 修复后同一路径转成 `unsupported_main_body: value coercion`。
- 已新增 IR 回归 `ordinary_callee_resume_site_drops_unreachable_suffix_after_nested_return`。
- 已完成验证：
  - `cargo fmt --check`
  - `cargo test -p scoopc ordinary_callee_resume_site_drops_unreachable_suffix_after_nested_return -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`：`T3016i` 标记为完成，下一项为 `T3016iR`。

## 备注

- 如果遇到规范不匹配、缺失语言特性或必须先修复的既有问题，不绕过，改为先在 `TODO.md`/`PLAN.md` 中建立前置任务并提交。
- 本轮严格只完成一个任务或一次必要的任务重排。
