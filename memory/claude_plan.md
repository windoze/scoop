## 当前任务

- 目标：完成 `TODO.md` 中首个未完成任务 `T2003c0b2c3d3`，然后停止。
- 已知上下文：前一轮工作已经在 `crates/scoopc/src/llvm/codegen/effect.rs` 中接入了一部分 `while` body 内 single direct + single indirect mixed-site continuation replay 支持，但尚未完成状态机所有路径，也未完成最终验收、文档更新和提交。
- 约束：如果发现该任务依赖尚未实现的语言特性、运行时能力或规范不匹配，必须先把阻塞项前置到 `TODO.md` / `PLAN.md`，提交后停止，不能绕过。

## 已知事实

- 最新摘要显示：尚未发现最新提交说明中的额外遗留问题，但仍需在正式执行前重新核对最新提交信息。
- 当前改动集中在 `crates/scoopc/src/llvm/codegen/effect.rs`。
- `TODO.md` / `PLAN.md` 尚未更新，当前任务也还没有提交。
- 之前中途有一次 `cargo check -p scoopc --features llvm` 通过到只剩 warning；后续又继续接了逻辑，因此当前编译状态未知。

## 执行计划

1. 读取最新提交说明、`TODO.md`、`PLAN.md` 与 `effect.rs` 当前状态，确认没有遗漏的 pre-existing issue，并重新锁定首个未完成任务确实仍是 `T2003c0b2c3d3`。
2. 在 `effect.rs` 中完成 while mixed-site 逻辑缺口，重点检查并补齐：
   - `state0` 路径；
   - `state1` 路径；
   - step trampoline 中 direct/indirect mixed 续跑尾部；
   - 与 `if_mixed_site_pcs_by_stmt_idx` 对应的 while 镜像分支是否齐全；
   - `used_after` / body-lift / prefix replay 的 while-specific helper 是否一致使用。
3. 先跑最小编译验证：`cargo check -p scoopc --features llvm`。
   - 若出现类型、借用、控制流或 warning 问题，立即修复直到通过。
4. 根据实现结果补充/调整 fixtures：
   - 为 while mixed direct+indirect 的支持路径新增 run-pass；
   - 为不支持边界保留或新增 build-fail；
   - 若发现规范边界与当前任务假设不一致，转为更新 `TODO.md` / `PLAN.md` 并停止。
5. 跑完整验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
6. 通过后更新：
   - `TODO.md`：把当前任务标记完成；
   - `PLAN.md`：记录完成情况与剩余计划；
   - 本文件：记录关键步骤完成情况和必要的计划调整。
7. 提交一次 git commit，然后停止，不继续做下一个任务。

## 风险与检查点

- 风险 1：while mixed-site 续跑在 `state0` / `state1` / resumed main tail 的控制流分支可能没有完全镜像 block/if，导致编译错误或运行时恢复错误。
- 风险 2：新加 helper 可能只接入了 step path，没有接入其他恢复阶段，导致特定 fixture 通过、全量测试失败。
- 风险 3：如果 while body mixed-site 的语义实际上需要跨 statement 支持，而当前实现只覆盖 same-body-stmt，则需要先回到任务拆解，而不是把缩小语义范围当作完成。

## 进度记录

- 已完成：按要求先写入本计划文件。
- 已完成：已核对最新提交、`TODO.md`、`PLAN.md` 与当前工作树状态，确认本轮首个未完成任务仍是 `T2003c0b2c3d3`，且最新提交说明没有额外 pre-existing issue。
- 已完成：已在 `crates/scoopc/src/llvm/codegen/effect.rs` 为 mixed escape matrix 的 `state0` / `state1` 补上 `while_mixed_site_pcs_by_stmt_idx` 分支，沿用现有 direct/indirect intercept 协议并接入 while 专用调度 helper。
- 已完成：`cargo check -p scoopc --features llvm` 已通过，`state0` / `state1` 接线没有引入新的编译错误。
- 已完成：已新增本任务 fixtures（2 个 run-pass + 1 个 build-fail）并手工试跑。
- 已完成：已定位并修复 post-immediate `while` mixed direct→indirect 的 loop re-entry 缺口。根因是 current indirect 完成后缺少“下一迭代从 sibling direct site 重新进入”的 dedicated lowering，且新 helper 初版少了一层 while-body scope，导致 `i` 在重新检查 loop condition 前被误弹栈。现已补专用 helper 并修正 scope 生命周期。
- 已完成：已补 `.stdout`、复测 targeted run-pass/build-fail，并通过以下全量验收：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 进行中：更新 `TODO.md` / `PLAN.md` / 本文件的完成状态，随后提交本轮变更并停止。
