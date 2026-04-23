# 当前执行记录

## 说明

按要求先写入计划文件。这里记录的是可执行步骤、检查项、决策依据摘要和进度更新，不包含不可审计的内部推理细节。

## 初始目标

本轮只完成一件事：

1. 先检查最新提交是否提到已有问题；若提到，优先修复。
2. 读取 `TODO.md`，找到第一个未完成任务。
3. 评估该任务是否过大；若过大，则拆分任务并更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。
4. 对当前目标任务完成实现、测试、文档更新、提交。
5. 完成后立即停止，不进入下一项任务。

## 执行步骤

1. 查看最新提交信息，确认是否显式提到待修复的既有问题。
2. 查看工作区状态，避免误覆盖用户已有改动。
3. 读取 `TODO.md` 与 `PLAN.md`，确定当前优先级和依赖关系。
4. 若执行过程中发现任何既有缺陷、规格不一致、回避式实现或实现边界缺口：
   - 先判断该问题是否阻塞当前任务。
   - 若阻塞且无法在本轮直接修复，则把前置修复任务插入 `TODO.md` 当前任务之前，更新 `PLAN.md`，提交后停止。
   - 若可直接修复，则先修复该问题，再继续当前任务。
5. 实现当前任务后运行相关验证：
   - 最小相关测试；
   - 必要时运行更广的回归；
   - 运行格式化、`cargo clippy --all-targets -- -D warnings`（若与任务改动相关且成本可接受）；
   - 确保无新增警告和回归。
6. 更新 `TODO.md`、`PLAN.md` 与本文件。
7. 提交 Git，提交信息对应任务编号或变更内容。

## 风险与约束

- 不回退或覆盖非本次任务引入的已有修改。
- 不采用规避式实现；遇到缺失特性或错误行为时，要么修复，要么显式前置成任务。
- 如果发现 `PROMPT.md` 存在意外改动，需要纳入提交而非忽略。

## 进度

- 已完成：创建计划文件并记录执行顺序。
- 已完成：检查最新提交、工作区状态、`TODO.md` 与 `PLAN.md`。
- 结论：
  - 最新提交 `d699c2cf6bcb294c9a71c5cb540ef8be70e6aaa6` 的提交信息仅为“`[T4016T2] Move task driver and state back into Scoop`”，未显式提到需先修复的既有问题。
  - 当前工作区只有本文件改动。
  - 顶层 `T4016` / `T4012` / `T4013` 等 `[TODO]` 为父任务；按顺序需要执行的第一个未完成叶子任务是 `T4016T3`。
- `T4016T3` 当前目标摘要：
  - 删除 `scoop_task_create` / `scoop_task_poll` / `scoop_task_step_ready` / `scoop_task_step_pending` / `scoop_task_from_result` / `scoop_task_join`；
  - 删除 `runtime/c/scoop_task.c`；
  - 移除 LLVM codegen 中针对 legacy task ABI 的 special-case；
  - 同步 `SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_TASK.md`、`sysroot/core.scoop` 到“task 只依赖 ordinary Scoop 定义 + generic continuation/sync substrate”的最终叙事。
- 初步热点位置：
  - `runtime/c/scoop_task.c`
  - `runtime/c/scoop_runtime_api.h`
  - `crates/scoop_runtime/build.rs`
  - `crates/scoop_runtime/tests/task_spawn_join.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`
  - `sysroot/core.scoop`
  - `SCOOP_RUNTIME.md`
- 已完成：阅读关键代码并确认 `T4016T3` 可直接执行，不需要再拆子任务。
- 当前实现判断：
  - 生产路径中仍残留 `Task.step()` -> `scoop_task_poll` 的 LLVM special-case。
  - `__scoop_task_create` / `__scoop_task_from_result` / `__scoop_task_join` / `__scoop_task_step_ready` / `__scoop_task_step_pending` 只剩 codegen 与 sysroot 声明债务；HIR lowering 已改用 `__task_*`。
  - `runtime/c/scoop_task.c` 与 `crates/scoop_runtime/tests/task_spawn_join.rs` 只服务即将删除的 task-only C ABI。
- 当前实施计划：
  1. 删除 sysroot 中 legacy `__scoop_task_*` 声明与相关注释，保留 `__task_*` 和 transport intrinsic。
  2. 删除 LLVM codegen 中 `scoop.core.step` / `__scoop_task_*` 的 task-only runtime special-case，以及对应 runtime symbol/ABI 声明。
  3. 删除 `runtime/c/scoop_task.c`，并从 runtime build / export allowlist 中移除相应符号。
  4. 删除或重写直接测 `scoop_task_*` ABI 的 runtime 测试；补充面向当前最终合同的编译器/夹具断言，确保 IR 不再引用 `scoop_task_*`（尤其补上 `scoop_task_poll`）。
  5. 同步 `SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、必要的 `SCOOP_FULL_SPEC.md` / `TODO.md` / `PLAN.md` 叙事。
  6. 跑定向测试，再跑全量验收命令。
- 下一步：开始修改 sysroot / codegen / runtime 文件。

## 本轮结果

- 已完成实现：
  - 删除 `runtime/c/scoop_task.c`。
  - 删除 `runtime/c/scoop_runtime_api.h` 中 `scoop_task_*` 导出名单。
  - 删除 `crates/scoop_runtime/build.rs` 中对 task-only runtime C 文件的编译入口。
  - 删除 `sysroot/core.scoop` 中 legacy `__scoop_task_*` 声明，并清理 `sysroot/task.scoop` 注释。
  - 删除 LLVM codegen 中 `scoop.core.step` / `__scoop_task_*` special-case，以及 `runtime_symbols.rs` / `runtime_abi.rs` 里的 `scoop_task_*` 符号声明。
  - 删除直接测试旧 ABI 的 `crates/scoop_runtime/tests/task_spawn_join.rs`。
  - 新增 LLVM 回归 `task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi`，补锁 `Task.step()` 不再走 `scoop_task_poll`。
  - 同步 `SCOOP_RUNTIME.md`、`SCOOP_TASK.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`TODO.md`、`PLAN.md`。

- 已完成验证：
  - `cargo fmt`
  - `cargo test -p scoopc --features llvm`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

- 结果判断：
  - `T4016T3` 已完成，可在 `TODO.md` 中标记为完成。
  - 当前下一项未完成任务已推进为 `T4012b3`。

- 下一步：复核 diff、提交 Git，并停止本轮。
