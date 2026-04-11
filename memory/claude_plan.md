# 本轮执行计划

说明：用户要求记录“完整思考过程”。我不会写入不可审计的内部推理细节，但会持续记录可执行计划、关键判断依据、已完成步骤、遇到的问题和后续决策，便于随时检查进度。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，完成后更新计划与任务状态，执行相关测试，提交一次 git commit，然后停止。

## 执行步骤

1. 检查最新一次 git commit 的提交信息与变更内容，确认其中是否提到已知问题、遗留缺陷或需要顺手修复的事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖、阶段目标和可能的拆分方案。
4. 判断该任务是否足够小且可在本轮完整交付。
5. 如果任务过大：
   - 将其拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把子任务插入正确位置；
   - 选择第一个子任务作为本轮目标。
6. 阅读与目标任务直接相关的代码、测试、文档与夹具，确认实现边界。
7. 实现该任务，必要时同步补充注释、文档或测试。
8. 运行针对性测试，然后运行要求的质量检查，至少包括：
   - 相关单元/集成/fixture 测试；
   - `cargo fmt --check`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 其他因任务影响而必须运行的验证命令。
9. 若测试失败或出现警告，先修复，再重新验证。
10. 更新进度文档：
    - 在 `TODO.md` 中将本轮完成的任务标记为完成；
    - 在 `PLAN.md` 中记录当前状态与后续影响；
    - 在本文件中补充关键结果与偏差说明。
11. 检查 git 工作区，确保变更范围合理，不回退用户已有改动。
12. 以清晰的提交信息提交本轮变更，然后停止。

## 当前已知信息

- 已检查最新一次提交：`06d05c494e208e646af0866ca6a4f354192ef862`，提交信息为 `[T2003c0b2b2] Support mixed-arm post-immediate indirect/direct escape site matrix`。
- 最新提交的信息中未额外声明需要先修复的遗留 issue；后续仍会在阅读相关代码与回归时留意是否存在明显回归。
- 已读取 `TODO.md` 与 `PLAN.md`。
- 已定位首个未完成任务：`T2003c0b2b3`，主题为“mixed-arm sibling escape-continuation 支持 pre-immediate top-level sites”。
- 已完成复杂度评估：当前任务虽然涉及 state machine 扩展，但边界仍然清晰，暂不需要再次拆分。

## 进度记录

- 已创建本文件，并写入初始计划。
- 已确认本轮目标候选为 `T2003c0b2b3`，下一步将检查相关 LLVM mixed-arm lowering、现有稳定诊断与 fixtures，判断是否可以在本轮完整交付。
- 已完成实现方案审计，当前判断如下：
  - `codegen_handle_expr_immediate_resume_with_escape_sibling_site_matrix` 已具备 direct/indirect top-level site matrix 的主体框架，但它把所有 `idx <= perform_idx` 的 escape sites 直接稳定诊断掉，因此 pre-immediate 仍未接入。
  - 该函数的 heap state 与 step trampoline 已经可以在多次 escape 之间保存/恢复 outer/body captures；真正缺的是“resume 某个 pre-immediate escape site 后，step trampoline 仍能在后续语句中重新命中 immediate-resume site，并在 immediate arm 完成后继续 replay”。
  - 入口选择逻辑也需要调整：只要存在 pre-immediate direct/indirect site，就应走 site-matrix lowering，而不是退回旧的 direct/indirect specialized path。
  - 现有负例 fixture `effect_resume_mixed_escape_pre_immediate_direct_indirect_is_error` 在本任务完成后应被替换为新的 run-pass 覆盖，并补一个更深层 shape 的稳定诊断负例。

## 具体实现计划

1. 调整 mixed-arm escape sibling 的入口分流：只要检测到 pre-immediate top-level direct/indirect site，就统一走 site-matrix lowering。
2. 扩展 site-matrix 扫描逻辑，允许收集 immediate site 之前的 top-level direct/indirect escape sites，而不是直接报 `perform before immediate site not yet supported`。
3. 扩展主路径 lowering：
   - 在 source-handle 的 `state0` 中加入 pre-immediate escape-site 拦截；
   - direct site 走 continuation alloc + escape arm；
   - indirect site 走现有 perform-slot dispatch + escape arm。
4. 扩展 continuation step trampoline：
   - 在 replay pre-immediate tail 时识别 top-level immediate site；
   - 在 step trampoline 内运行 sibling immediate-resume arm；
   - immediate arm `resume(value)` 完成后继续 replay immediate 之后的语句，并允许后续再次命中 escape sites。
5. 补 fixtures：
   - 新增 run-pass，至少覆盖 pre-immediate direct、pre-immediate indirect；
   - 保留一个更深层 body shape 的稳定诊断负例。
6. 跑测试与质量检查，之后更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 最终结果

- 已完成 `T2003c0b2b3`，未再继续处理后续任务。
- LLVM mixed-arm site-matrix lowering 现已支持：
  - pre-immediate top-level direct escape site；
  - pre-immediate top-level indirect escape site；
  - continuation step 在恢复 pre-immediate escape site 之后重新命中 sibling immediate-resume site，并在 immediate arm `resume(...)` 后继续 replay 后续 top-level tail。
- 已新增/调整 fixtures：
  - run-pass `effect_resume_mixed_escape_pre_immediate_direct`
  - run-pass `effect_resume_mixed_escape_pre_immediate_indirect`
  - build `effect_resume_mixed_escape_pre_immediate_nested_is_error`
  - 删除旧的 top-level pre-immediate 负例 `effect_resume_mixed_escape_pre_immediate_direct_indirect_is_error`
- 已更新 `TODO.md` / `PLAN.md`，将 `T2003c0b2b3` 标记为完成，并把下一步移动到 `T2003c0b2c`。

## 验证记录

- `cargo check -p scoopc --features llvm`
- `cargo fmt`
- `cargo test --all`
- `cargo run -p scoop -- test`
- `cargo run -p scoop --features llvm -- test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`

## 额外说明

- 在设计 pre-immediate direct 的 run-pass 回归时，发现“在 escape arm 中把 pre-immediate direct payload binder 直接拿去 `println` / 显式 `Int` 赋值”会触发单独的 print/coercion 缺口；这条问题与本任务要打通的“pre-immediate site replay + re-enter immediate-resume state machine”不是同一个缺口。
- 因此最终的 direct run-pass 回归聚焦在：
  - pre-immediate direct site 能正确返回 arm result；
  - continuation binder `k` 可保存并恢复；
  - continuation `resume(...)` 后能继续执行后续 immediate-resume site 与 tail。
- 如果后续要继续扩面，可以把“pre-immediate direct payload binder 在 arm body 中的 richer 使用”单独列成后续小任务，而不是把它和本轮的 control-flow/state-machine 改动混在一起。
