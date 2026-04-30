# 执行计划与进度记录（每次调用只完成 TODO.md 的第一个未完成任务）

说明：
- 按照安全要求，这里不记录“完整思考过程/逐字推理”，只记录可审阅的**执行计划、关键决策与进度**。
- 在执行过程中，如计划调整或完成关键步骤，会随时更新本文件，便于你检查进展。

## 总体流程（本次调用）

1. 检查最新提交
   - `git log -1` 查看最新提交信息与改动。
   - 若提交信息提到已知问题/回归/临时修复点：先复现并修复该**既存问题**，并补充测试。

2. 确认本轮要做的任务
   - 打开 `TODO.md`，定位**第一个未完成任务**（未勾选/未标记 Done）。
   - 若任务过大：拆分为可交付子任务，更新 `PLAN.md` 与 `TODO.md`（把新子任务插到原任务处），并以第一个子任务作为本轮执行目标。

3. 实现
   - 在遵循仓库约定（Rust 2024、模块结构、fixture 目录规则等）的前提下，实现该任务。
   - 过程中若发现任何既存 bug / spec 不一致 / 测试回归：立即优先修复；若无法立刻修复，则把“修复该问题”作为前置任务插入 `TODO.md` 并停止本轮。

4. 测试与质量门禁
   - 运行与改动相关的最小测试集合（单元/集成/fixture）。
   - 然后跑全量：`cargo test --all`。
   - 运行：`cargo clippy --all-targets -- -D warnings`，确保无警告。
   - 必要时 `cargo fmt`。

5. 文档与任务状态
   - 在 `TODO.md` 勾选/移动该任务到 Done。
   - 更新 `PLAN.md` 反映当前状态与后续依赖（若有）。
   - 更新本文件记录关键结果与运行过的命令。

6. 提交与停止
   - `git status` 确认变更。
   - 以清晰信息提交：形如 `[Txxxx] ...` 或按 TODO 中的任务标识。
   - 提交完成后立刻停止，不做下一个任务。

## 当前状态

- 已完成：
  - 查看最新提交：`eb2a8043`，标题为 `[T5001f8a] Close caller-side outer mutable writeback/readback`。
  - 读取 `TODO.md`，确认本轮首个未完成任务为：`T5001f8aR Review：确认 outer mutable local 的 caller-side source-of-truth 已闭合`。
  - Review 代码路径并完成验证（均通过）：
    - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
    - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_outer_mutable_writeback_basic.scoop`
    - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
    - `env SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
  - 文档更新：
    - `TODO.md`：将 `T5001f8aR` 标记为 `[DONE]` 并补充完成记录与验证命令。
    - `PLAN.md`：追加 `T5001f8aR` review 完成说明。

- 进行中：
  - 已提交：`[T5001f8aR] Review caller-side outer mutable writeback/readback`。
  - 本轮按要求已完成首个未完成任务并停止。
