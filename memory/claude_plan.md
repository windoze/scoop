## 当前执行计划（审阅版）

说明：按要求先记录执行计划。此文件只包含可审阅的任务分析摘要、执行步骤、风险和进度，不包含内部私有推理细节。

### 目标

完成 `TODO.md` 中第一项未完成任务，并在完成后更新计划/任务状态、运行相关测试、提交 Git commit，然后停止。

### 执行步骤

1. 检查最新一次 Git 提交，确认其是否提到任何已知遗留问题。
2. 如果最新提交提到需先修复的遗留问题，优先定位并修复这些问题，再继续后续步骤。
3. 阅读 `TODO.md`，识别第一项未完成任务。
4. 评估该任务规模与前置依赖：
   - 若任务可直接完成，进入实现。
   - 若任务过大或存在明确前置依赖/规格缺口，更新 `PLAN.md` 与 `TODO.md`，将任务拆分或重排，并只执行拆分后的第一项。
5. 阅读相关代码、规格、测试与文档，确定实现位置与影响范围。
6. 实现任务，确保不引入临时性 workaround，不偏离规格。
7. 运行相关测试与必要的质量检查：
   - 至少运行与改动直接相关的测试；
   - 若影响面较大，补充运行更广范围测试；
   - 按要求关注 `cargo clippy --all-targets -- -D warnings` 是否通过。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成，或在阻塞时按依赖顺序调整任务；
   - 更新 `PLAN.md` 记录当前状态与后续安排；
   - 如有必要，更新 `README.md` 或内联注释。
9. 检查工作区改动，整理提交内容。
10. 使用清晰的提交信息创建 Git commit，然后停止。

### 约束与判断原则

- 仅处理一个任务。
- 遇到规格缺口、实现边界或缺失特性时，不绕过；必须先在 `TODO.md`/`PLAN.md` 中显式建模依赖。
- 不回退用户已有改动；若工作区存在无关脏改动，仅在必要范围内协作处理。

### 进度记录

- 2026-04-20：已创建本计划文件，尚未开始仓库检查。
- 2026-04-20：已检查最新提交 `f8dec450b22345561c2517875f2bcaf82916a698`（`[T4009h] Close stable handle wake-token contract`），提交说明未额外引入需要先行修复的点名遗留问题。
- 2026-04-20：已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`，确认当前顺序上的首个未完成执行项是 `T4009R`（Review：确认 `Task` 本体已脱离 executor 前提）。
- 2026-04-20：当前执行焦点切换为 `T4009R`。下一步将：
  1. 全局检索残留的 `Executor` / `scoop.task.*` / 公开 `spawn/join` 主线依赖；
  2. 复审 `Task.poll()/step()`、async sugar、runtime task object model、stable handle / `Pinned` 职责边界；
  3. 运行针对性测试与质量检查；
  4. 若确认无剩余裂缝，则更新 `TODO.md` / `PLAN.md` / `ISSUES.md` 并提交。
- 2026-04-20：结构审计已完成。已复审 `sysroot/core.scoop`、`runtime/c/scoop_task.c`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoop_runtime/src/abi_exports_allowlist.rs`，并全局扫描 `crates/` / `runtime/` / `sysroot/` / `stdlib/`，确认当前主线没有公开 `scoop.task.*` / `Executor` surface 或 runtime executor implementation 残留；`spawn` / `join` 当前只保留 deferred 语法壳。
- 2026-04-20：验证已完成，结果全绿。已运行：
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo run -q -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 2026-04-20：已将 review 结论写回 `TODO.md` / `PLAN.md` / `ISSUES.md`：`T4009R` 与总任务 `T4009` 已标记完成，`ISSUES.md` 第 2 条已改写为“已收口”，下一项已前移到 `T4010a`。下一步只剩整理工作区并创建本轮 commit。
