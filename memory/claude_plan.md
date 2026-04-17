# 执行计划与决策记录

## 说明

按要求，我会在这个文件里持续记录执行计划、关键进展、以及必要的决策摘要。
出于安全与协作边界考虑，这里记录的是可审计的决策过程摘要、假设、步骤和结果，而不是不加筛选的内部思维流。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 git 提交，确认提交信息里是否提到任何已知问题；如果有，先修复这些问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，确认现有计划与任务依赖关系。
4. 如首个未完成任务过大或存在隐藏依赖，先把任务拆成更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本轮只执行拆分后的第一个子任务。
5. 实现该任务，并补充或调整必要测试。
6. 运行相关验证，至少覆盖：
   - 受影响范围的定向测试
   - 必要时运行更广的 `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 必要时 `cargo fmt --check` 或 `cargo fmt`
7. 更新文档与计划：
   - 在 `TODO.md` 中标记任务完成，或如被阻塞则按要求重排任务
   - 在 `PLAN.md` 中记录状态变化、依赖、阻塞原因（如有）
   - 持续更新本文件，记录关键进展
8. 使用清晰的提交信息提交本轮改动。
9. 停止，不继续执行下一个任务。

## 当前假设

- 仓库可能存在未提交改动，因此任何修改前都需要检查 `git status`，避免覆盖用户已有工作。
- “最新提交中提到的问题” 需要通过检查最近一次提交信息以及必要时查看其改动内容来判断。
- 如果实现过程中发现规范不匹配、缺失语言特性或依赖性缺陷，不能绕过，必须先更新 `TODO.md` / `PLAN.md` 反映真实依赖，再决定本轮是否转为处理该前置问题。

## 待确认事项

- 最新提交是否声明了尚未修复的问题。
- `TODO.md` 中第一个未完成任务的范围和复杂度。
- 当前工作树是否干净，以及是否存在与本轮任务冲突的用户改动。

## 进度

- 已创建本计划文件，等待开始仓库检查。
- 已检查工作树：当前只有本文件处于未提交状态。
- 已检查最新提交 `f69965eef95bd81cbbbf2003882c9bd7e4824365`：该提交未修复生产问题，而是把一个新的前置 blocker 记录为 `T3009b0a1c` / `T3009b0a1cR`。
- 已读取 `TODO.md` / `PLAN.md` 并定位首个未完成任务：`T3009b0a1c`「修正 unified SuspendCall 的 inactive-continue / active-dispatch 合同」。

## 当前任务判断

- 当前首个未完成任务边界明确，暂不需要进一步拆分。
- 预期改动集中在 unified state-machine emitter / terminator 的 call-boundary 路径，重点检查：
  - `state_machine_emitter.rs`
  - 可能涉及的 contract / segment / plan 结构
  - 相关定向 fixture 与 IR / 单元测试

## 当前任务执行计划

1. 阅读 `SuspendCall` 相关生产代码，确认当前 inactive-path 为什么会被无条件建模成 suspend。
2. 运行最小复现或已有定向 fixture，确认失败形态并锁定入口。
3. 修改统一 lowering / emitter，让 `SuspendCall` 在 TLS inactive 时继续当前 state machine caller-tail，在 TLS active 时仍按统一 dispatch 路径返回。
4. 如缺少足够测试，补充或收紧与 `SuspendCall` inactive-path 直接相关的测试。
5. 运行定向验证，再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，提交本轮改动并停止。

## 当前任务完成情况

- 已定位根因：`HandleStateOp::SuspendCall` 自身只是正常求值，真正错误的是 `UnifiedStateTerminator::Suspend` 对 call-like suspend site 无条件执行 continuation alloc + set_active + return。
- 已完成生产修复：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - 对 `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` 三类 site，在 terminator 中先检查 TLS active。
  - inactive 时：把 call 结果写入 frame resume 槽并 branch 到 `resume_state`，继续当前 step function 内的 caller-tail。
  - active 时：保持原有 continuation alloc + outward dispatch 路径。
- 已补充缺失验收 fixture：
  - `tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.stdout`
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_suspend_call_inactive_helper_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_hidden_suspend_local_closure_helper_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

## 当前状态

- `T3009b0a1c` 已完成并已在 `TODO.md` 标记为 done。
- `PLAN.md` 已更新执行顺序，下一步为 `T3009b0a1cR`。
- 本轮剩余工作：
  1. 检查最终 diff 与 git 状态。
  2. 以本任务为主题提交一次 Git commit。
  3. 停止，等待下一轮执行 review 任务。
