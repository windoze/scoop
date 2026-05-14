## 当前执行计划

说明：按安全与协作要求，这里记录可执行计划与进度，不记录内部推理细节。

1. 读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果是，则将其视为当前任务的一部分或在 `TODO.md` 中补充为前置任务。
3. 阅读与当前任务直接相关的代码、测试、规范和任务说明，确认约束、依赖和验收要求。
4. 以最小正确改动实现当前任务；如果遇到会阻塞该任务的真实缺陷或缺失能力，则先修复它，或按要求在 `TODO.md` 中加入最小前置任务并停止。
5. 运行与当前任务相关的验证，并补充必要测试；至少覆盖任务要求，必要时执行更广的回归检查。
6. 更新文档与记录：
   - 在 `TODO.md` 中将当前任务标题标记为 `[DONE]`，并填写完成记录。
   - 仅在阶段计划发生变化时更新 `PLAN.md`。
   - 继续更新本文件记录关键进展或计划调整。
7. 按仓库约定创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度记录

- 已创建初始执行计划。
- 已读取 `TODO.md`，确认首个未完成任务为 `P0-T01`（冻结 current ABI baseline 与 regression owner map）。
- 已检查最近一次提交：仅更新计划文件，未发现需要先插入到 `P0-T01` 之前的直接相关未完成实现项。
- 当前执行重点：
  1. 读取 `PLAN.md` / `MANAGED_ABI.md` 的 P0 相关章节，核对 baseline 与设计 drift。
  2. 审计 `crates/scoopc/src/llvm/tests.rs` 与现有 fixture，补齐集中 ABI baseline audit。
  3. 运行 `P0-T01` 指定验证命令；若出现阻塞当前任务的真实回归，先修复或在 `TODO.md` 中补最小前置任务。
  4. 回写 `TODO.md` 完成记录与本文件，然后提交并停止。
- 已完成代码/文档改动：
  - 在 `crates/scoopc/src/llvm/tests.rs` 新增 `abi_baseline_*` 审计测试，并把 effectful bridge / sysroot string owner 测试收口到共享 helper。
  - 在 `MANAGED_ABI.md` 修正当前 native `FunPtr` aggregate-return 描述，并新增 `P0-T01` regression owner map + drift 记录。
- 已完成验证：
  - `cargo test -p scoopc abi_baseline -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build/extern_enter_native_no_statepoint_writeback.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/extern_fun_effectful_funptr_bridge_ok.scoop`
  - `cargo test -p scoopc llvm_tests -- --nocapture`（当前 filter 命中 0 测试）
  - `cargo test -p scoopc effectful_funptr_call_uses_explicit_outcome_boundary -- --nocapture`
  - `cargo test -p scoopc single_file_minimal_ir_includes_compilable_sysroot_string_helpers -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`
- 已回写 `TODO.md`：`P0-T01` 已标记为 `[DONE]`，并补充完成记录。
- 下一步：检查工作区改动并创建 `P0-T01` 提交，然后停止。
