# Claude Plan

## Note
- 按要求维护执行计划与进度记录。
- 不记录内部推理细节；此文件只保存可执行计划、发现的问题、决策与进度。

## Initial Plan
1. 检查最新一次提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务及其上下文。
3. 如果该任务过大，先细化并更新 `TODO.md` / `PLAN.md`，本次只执行新的第一个子任务。
4. 实现当前目标任务，避免规避既有缺陷；若发现阻塞问题，先修复或把前置任务插入 `TODO.md`。
5. 运行相关测试与必要的质量检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并按仓库约定创建一次提交，然后停止。

## Progress Log
- 已创建初始计划，准备开始检查仓库当前状态。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。
  - 最新提交 `[T5001e1] Reload GC refs from explicit frame after safepoints` 未显式记录需先处理的遗留问题。
  - 当前首个未完成任务是 `T5001e1R Review：确认 safepoint 已成为真实的 clobber 边界`。
- review 期间发现一处真实缺口：`crates/scoopc/src/llvm/codegen/mir_body.rs` 的 `load_mir_local(...)` 仍直接从 `slot.ptr` 读取，未走 `local_ptr_for_use(...)`，会让 production MIR body 在 safepoint 后继续读取旧 local 槽位。
- 已修复该缺口：MIR local load 现在也统一通过 `local_ptr_for_use(...)` 选择 post-safepoint reload 槽位。
- 已新增 production MIR LLVM 回归，锁定 raw/materialized MIR 经普通 managed call safepoint 后，direct GC local 会从 explicit frame home slot reload。
- 已完成验证：
  - `cargo test -p scoopc --lib`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`，准备整理本次提交。

## Current Task: T5001e1R
1. 审查 `T5001e1` 涉及的 lowering 路径与 LLVM 回归，重点检查 ordinary call、runtime helper、effect boundary、resume replay 后是否仍可能直接复用 safepoint 前 GC SSA / register 值。
2. 若发现真实 correctness 缺口，先修复缺口并补回归，再继续本次 review 结论整理。
3. 运行与本任务相关的测试与质量检查。
4. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 review 结论，然后提交一次 git commit 并停止。
