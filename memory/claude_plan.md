# 当前执行计划

说明：这里记录的是可见的执行计划、判断依据摘要与进度更新，不包含逐字内部思维。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，并在完成后停止。

## 约束与执行顺序

1. 先检查最近一次提交，确认是否提到已有问题；若有，先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如果该任务过大，先拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或子任务。
5. 运行相关测试与必要的质量检查，至少覆盖受影响范围；若任务完成，尽量补充更全面验证。
6. 更新文档状态：
   - 在 `TODO.md` 中标记完成，或在阻塞时按要求重排任务。
   - 在 `PLAN.md` 中记录当前状态、依赖和后续顺序。
   - 持续更新本文件，记录关键步骤与计划变化。
7. 提交 git commit，然后停止，不继续做下一个任务。

## 预期检查项

- 最近一次提交的提交信息与变更摘要
- `TODO.md`
- `PLAN.md`
- 如有必要，`README.md`、相关源码、相关测试和规范文档

## 风险与处理原则

- 如果发现规范缺口、实现缺口或测试依赖缺失，不能绕过，必须先把前置修复任务写入 `TODO.md` 并调整顺序。
- 不回退用户已有修改；若遇到冲突，先理解现状，再只改动完成当前任务所需部分。
- 任何关键进展、计划变更、阻塞原因，都会同步更新到本文件。

## 进度记录

- 已创建本计划文件，接下来将检查最近一次提交与任务清单。
- 已检查最新提交 `3b8f29f`，提交标题为 `[T3009b0a1b] Add resumed outer-slot writeback fixture`，无正文说明额外遗留问题。
- 已定位 `TODO.md` 中首个未完成任务为 `T3009b0aR`：Review「确认 outer-scope slot 写回没有回流成 effect-only patch」。

## 当前执行计划（针对 T3009b0aR）

1. 阅读 `TODO.md` 中 `T3009b0aR` 及其直接前置任务 `T3009b0a` / `T3009b0a1a` / `T3009b0a1b` 的描述。
2. 检查实现 outer-slot 写回的生产代码与相关测试，确认写回触发点、适用边界和非 effect 路径是否共享同一合同。
3. 运行定向测试；必要时补充更小的 repro 来验证 “inactive / resumed completion / non-effect path” 等边界。
4. 若发现生产问题，立即修复并重新验证；若未发现问题，则整理审查结论。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交 commit，然后停止。

## 当前审查结论（进行中）

- outer-slot seeding / writeback 的生产实现集中在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`。
- `seed_outer_scope_frame_slots` 会在 handle 入口把 outer mutable slot 的原始 storage pointer 记录进 frame metadata。
- `write_back_outer_scope_frame_slots` 只从 frame metadata 读取 authoritative target，不依赖 caller `env`。
- 统一 step-function 返回出口（`ReturnHandle` / `ReturnFromFunction` / `Suspend` / `ArmReturnHandle` / `ArmResumeMatchedSite` / `ArmMaterializeContinuation`）以及 `handle_done` / `handle_propagate` 都复用同一个 writeback helper。
- `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中 `codegen_continuation_resume_builtin` 仅负责 payload transport、调用 `scoop_continuation_resume` 和 ordinary effect propagation check，没有 outer-slot 写回逻辑。
- 下一步：用定向 IR/fixture 与全量测试验证以上审查结论。

## 完成情况

- 复审完成，未发现需要修复的生产代码问题。
- 已确认 outer-slot 写回没有回流成 effect-only patch：
  - outer-slot authoritative source/target 都由 unified handle frame metadata 驱动；
  - `ReturnHandle` / `ReturnFromFunction` / `Suspend` / `Arm*` 返回出口与 `handle_done` / `handle_propagate` 共用 `write_back_outer_scope_frame_slots`；
  - `codegen_continuation_resume_builtin` 不承担 outer-local 同步职责。
- 已完成验证：
  - `cargo test -p scoopc handle_outer_scope_seeding_includes_arm_and_finally_locals -- --nocapture`
  - `cargo test -p scoopc escaped_continuation_resume_ir_records_outer_slot_storage_and_writeback -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_outer_var_writeback_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_outer_var_writeback.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`，下一项未完成任务推进为 `T3009b0R`。
