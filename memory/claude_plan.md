# 本轮执行计划

更新时间：2026-04-18

## 目标

按 `TODO.md` 的顺序执行第一个未完成任务，并在完成后停止。

## 初始计划

1. 检查最新一次 Git 提交，确认是否提到了已知遗留问题。
2. 如果最新提交提到了需要先修复的遗留问题，优先定位并修复这些问题。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 阅读 `PLAN.md`，核对当前计划与任务依赖关系。
5. 评估该任务是否足够小且可以在本轮完整交付。
6. 如果任务过大，则把它拆分为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，本轮只执行新的第一个子任务。
7. 实现本轮目标任务。
8. 运行与变更相关的测试，并补充必要测试。
9. 运行质量检查，至少覆盖 `cargo fmt --check`、相关测试，以及尽量覆盖 `cargo clippy --all-targets -- -D warnings`。
10. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
11. 提交本轮修改，提交信息与任务编号对应。
12. 停止，不继续处理下一个任务。

## 约束

- 不接受规避式修复、临时兼容层或仅对夹具生效的补丁。
- 如果发现规范不匹配、功能缺失或实现边界不完整，必须先在 `TODO.md` / `PLAN.md` 中建模为依赖任务，再决定是否继续。
- 不回退用户已有修改；如果工作区存在无关脏改动，只在理解其影响后绕开处理。

## 进度记录

- 已创建本轮计划文件。
- 已检查最新提交 `cde337f37f0f90d9e92c43cbe269bd73f0df8f86`（`[T3016aR] Review cleanup completion contract`）。提交信息未单独声明新的遗留问题需要先修。
- 已确认 `TODO.md` 中首个未完成任务为 `T3016b`：修正 escaped continuation resumed-body tail replay 在 block/when/loop 混合控制流中的回归。
- 已完成定向复现：
  - `effect_escape_continuation_perform_in_when_arm.scoop`：恢复后先执行正确 arm tail，再错误重放 enclosing `when`，重复打印 `before_ask` / `after_ask`。
  - `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`、`effect_multi_escape_direct_indirect_while.scoop`：后续 `resume(...)` 丢失 direct 之后、indirect 之前的 prefix。
- 当前判断：原 `T3016b` 范围过大，至少包含两个独立缺口，已据此拆分任务。
- 已更新 `TODO.md` / `PLAN.md`：
  - 新增前置任务 `T3016b0` / `T3016b0R`，专门修正 statement-position `when` arm resumed-body 恢复后重放 enclosing `when` 的回归。
  - 原 `T3016b` / `T3016bR` 收窄为 block/if/while mixed direct+indirect 路径中的 resumed-segment prefix replay 缺口。
- 当前本轮执行目标已切换为新的首个未完成任务 `T3016b0`。
- `T3016b0` 已实现完成：
  - `state_machine_plan.rs` 的 `materialize_resume_fragments()` 继续执行 resume-slot rewrite，但会在 statement-position `when` arm 的恢复态里剔除已被 arm tail 覆盖的 enclosing `WhenExpr`。
  - 新增结构测试 `source_plan_elides_enclosing_when_expr_after_when_arm_resume`。
  - `tests/fixtures/run-pass/effect_escape_continuation_perform_in_when_arm.scoop` 已改回 `EXPECT: pass`。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_perform_in_when_arm.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前待收尾：检查工作区、确认 `TODO.md` / `PLAN.md` 已标记完成，然后以 `T3016b0` 对应提交信息提交并停止。
