# 当前执行计划

## 约束说明

- 本次调用只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在真正开始实现前，先检查最新提交是否提到遗留问题；如果有，先修复这些问题。
- 计划会随着检查结果、实现进展和阻塞情况持续更新。
- 这里记录的是可审计的执行计划、判断依据摘要和进展，不包含逐字内部推理。

## 初始步骤

1. 检查最新一次 Git 提交：
   - 查看提交信息与可能关联的改动范围。
   - 判断提交是否明确提到已有缺陷、已知问题或待补修内容。
   - 如果存在这类问题，先把这些问题纳入本次处理范围，并优先修复。
2. 阅读 `TODO.md`：
   - 找出第一个未完成任务。
   - 判断任务是否足够小，能在一次调用内完整实现、测试、记录并提交。
3. 如任务过大：
   - 拆分为更小的子任务。
   - 更新 `PLAN.md`，记录新的分解和依赖关系。
   - 更新 `TODO.md`，把原任务替换或扩展为子任务，并确保第一个子任务成为当前执行目标。
4. 实现当前目标任务：
   - 先阅读相关代码、测试、规范和上下游模块。
   - 识别是否存在规范缺口、实现边界或历史 workaround。
   - 如果发现阻塞当前任务的真实缺陷或缺失特性，不绕过，转而把缺陷前置为新的 `TODO.md` 任务并调整计划。
5. 验证：
   - 运行与改动直接相关的测试。
   - 运行必要的质量检查，至少覆盖构建、相关测试，以及在可行范围内执行 `cargo clippy --all-targets -- -D warnings`。
6. 文档与状态同步：
   - 更新 `TODO.md` 的任务状态与顺序。
   - 更新 `PLAN.md` 说明当前状态、完成情况或阻塞原因。
   - 按关键节点继续更新本文件。
7. 提交：
   - 使用清晰的 Git 提交信息提交本次变更。
   - 提交后停止，不继续处理下一个任务。

## 待确认信息

- 最新提交是否声明了遗留问题。
- `TODO.md` 中第一个未完成任务是什么。
- 当前任务是否需要先拆分，或是否被规范缺口阻塞。

## 进展记录

- 已创建本计划文件，接下来进入仓库检查阶段。
- 已检查最新提交 `cf4fce5 [T2003r3d2c2] Reconnect heap-continuation-only multi-resuming leaf`：
  - 提交信息本身未声明额外待修遗留问题。
  - 当前工作树除本计划文件外无未提交改动。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T2003r3d2c3`：
  - 任务目标：接回 unified multi-resuming leaf 的当前 legal `1 immediate + 1 escape` mixed 基线。
  - 该任务当前的直接缺口位于 `crates/scoopc/src/llvm/codegen/effect/nonresuming.rs`，`MultiResuming` 入口对 `immediate_arms.len() == 1 && escape_arms.len() == 1` 仍返回 “not yet connected”。
- 已完成可执行性判断：
  - 暂不需要继续拆分 `TODO.md`。
  - 当前更像是“接线缺口”而不是新的规格不明问题。
  - unified metadata helper 已存在，尤其是 `resolve_immediate_resume_with_escape_sites_from_plan(...)`，说明计划恢复层已具备 mixed leaf 所需输入。
- 当前实现计划细化如下：
  1. 新增一个 unified mixed multi-resuming leaf（优先放在独立模块文件中，避免继续膨胀现有文件）。
  2. 在 `MultiResuming` 入口中把 `1 immediate + 1 escape` 路由接到该新 leaf。
  3. 复用现有 shared helper 处理 plan 恢复、capture 元数据与 sibling non-resuming dispatch，避免恢复任何已删除的 shape-based scanner/route。
  4. 把现有“pending”定向单测改成成功样例，并补一个 representative run-pass fixture，至少覆盖 mixed 路径与 `finally` 或 sibling non-resuming 中的一种组合。
  5. 运行定向 `cargo test` / fixture / `cargo clippy`，然后再更新 `TODO.md`、`PLAN.md` 并提交。
- 实现结果：
  - 已新增 `crates/scoopc/src/llvm/codegen/effect/multi_resuming_mixed.rs`，承接 unified `1 immediate + 1 escape` mixed leaf。
  - 已在 `crates/scoopc/src/llvm/codegen/effect/nonresuming.rs` 中把 `counts.stack_reenter == 1 && counts.heap_continuation == 1` 且无 sibling non-resuming arm 的 multi-resuming route 接到该 leaf。
  - 本轮 representative 组合选择 `finally`：leaf 现已打通 top-level direct immediate site、top-level direct escape site、heap state capture、延后 `k.resume(...)` replay 与 `finally` cleanup。
  - 现阶段 mixed + sibling non-resuming 的更广 coverage 没有在本轮继续扩展，仍由后续 `T2003r3d3` 承接。
- 回归与质量验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc unified_multi_resuming_codegen_emits_single_immediate_single_escape_finally_sample -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_escape_direct_finally.scoop`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - 上述命令均已通过。
- 文档状态：
  - 已将 `TODO.md` 中 `T2003r3d2c3` 标记为完成，并写入完成说明。
  - 已更新 `PLAN.md`，记录本轮 mixed leaf 的真实落点与下一步为 `T2003r3d3`。
