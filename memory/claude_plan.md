# 执行计划（单次调用：完成 TODO.md 中第一个未完成任务）

说明：我会在此文件记录“可复现的执行步骤、检查点与进度”，不记录任何私有推理细节。

## 总体流程（每次调用固定）

1. 检查最新提交信息：`git log -1`。
   - 若提交信息提到已知问题/回归/临时 workaround：先定位并修复该问题，再继续。
2. 打开 `TODO.md`，定位第一个未完成任务。
3. 评估复杂度：
   - 若任务过大：把它拆成可在一次调用内完成的小任务；同步更新 `PLAN.md` 与 `TODO.md`（新子任务按依赖顺序插入，当前要执行的是第一个子任务）。
4. 实施：完成当前要做的任务（尽量做最小正确改动）。
5. 验证：
   - 至少运行：`cargo test --all`。
   - 并运行：`cargo clippy --all-targets -- -D warnings`。
   - 若涉及 fixtures/spec：按仓库指引运行相应 `scoop_tools`/fixture suite。
6. 文档与追踪：
   - 将该任务在 `TODO.md` 标记完成。
   - 更新 `PLAN.md` 反映当前状态、依赖关系与任何调整。
   - 如执行过程中发现与 spec 不符/缺失特性且阻塞：把“修复该问题”的新任务插入到 `TODO.md` 的依赖位置，更新 `PLAN.md`，提交并停止（本次不继续原任务）。
7. 提交：
   - 按仓库风格写清晰提交信息（例如 `[Txxxx] ...`）。
8. 停止：不继续下一个任务，等待下一次调用。

## 本次调用进度

- [x] 读取最新提交信息，确认是否有需优先修复的问题
- [x] 读取 `TODO.md`，选定第一个未完成任务
- [x] （必要时）任务拆分并更新 `PLAN.md`/`TODO.md`
- [x] 实施任务
- [x] 运行测试与 clippy
- [x] 更新 `TODO.md`/`PLAN.md`
- [x] Git 提交并停止

## 本次调用结果

- 已完成任务：`T5001f8b`
- 提交：`973dedd0`（`[T5001f8b] Materialize stable exec homes for state-machine frame locals`）
- 关键验证：
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
