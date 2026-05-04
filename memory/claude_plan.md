# 本次执行计划

说明：按安全约束，这里记录可审计的高层推理摘要与执行计划，不写出逐字内部思维。

1. 先读取 `TODO.md`，把它当作索引使用。
2. 按 `TODO.md` 引用顺序读取对应的 `TODO-Px.md` 详细任务文件。
3. 依据任务标题是否带有 `[DONE]`，定位第一个未完成的详细任务；若 `TODO.md` 与详细文件不一致，以详细文件为准。
4. 检查最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题；若存在，将其视为当前任务的一部分或必要前置。
5. 阅读当前任务的详细要求、约束、依赖、验证标准，并结合代码库现状确认实现边界。
6. 实现当前任务；若遇到阻塞当前任务且不能规避的真实缺口，先修复该缺口，或在对应 `TODO-Px.md` 中插入最小必要前置任务并同步 `TODO.md`，随后停止。
7. 运行与当前任务直接相关的测试、构建、格式化和 lint（至少覆盖任务要求及必要回归范围），修复发现的问题，目标是无警告通过，包括 `cargo clippy --all-targets -- -D warnings`（若适用且不与任务上下文冲突）。
8. 更新文档记录：
   - 在对应 `TODO-Px.md` 中把当前任务标题标记为 `[DONE]`，并填写/更新完成记录。
   - 若任务索引、标题、顺序或新增前置任务发生变化，同步更新 `TODO.md`。
   - 仅当阶段计划本身变化时才更新 `PLAN.md`。
9. 复查工作区，确保不回退或覆盖他人未请求的更改；若是恢复上次失败留下的同一任务未提交工作，则与本次变更一并提交。
10. 使用清晰的 git 提交信息提交当前任务成果，然后停止，不进入下一个任务。

进度更新规则：
- 定位到当前任务后，补充任务编号、目标与验证方案。
- 开始编辑前，记录计划是否有调整。
- 完成关键实现、测试、文档更新、提交前，各追加一次简要进度记录。

## 进度记录

### 2026-05-04 当前任务确认

- 当前按 `TODO.md` 索引与完成标记核对后，第一个未完成详细任务是 `TODO-P6-part2.md` 中的 `P6-T02qc`。
- 最新提交标题为 `[P6-T02qc] Record plan log update`，与当前任务直接相关，因此需要把当前工作区视为该任务的可能续做现场，检查是否存在未提交实现并在完成后一并提交。
- 当前目标：发布 shared surface-resume wrapper 的 owner-step -> wrapper-step 投影 contract，使后续 `P6-T03` 不必在 shared surface body 中反向推导 wrapper 返回语义。
- 初步验证方案：
  - 读取 `TODO-P6-part2.md` 中 `P6-T02qc` 的详细约束与完成标准。
  - 检查 refactor late-lowered / LLVM ABI query / surface-resume 相关实现与现有测试。
  - 为 owner-step -> wrapper-step projection 增加 authoritative handoff/query 与 fail-fast。
  - 运行定向单元测试、相关 build/fixture 测试，以及必要的 `cargo clippy --all-targets -- -D warnings`/格式化检查。

### 2026-05-04 设计收敛

- 现状确认：`P6-T02q` 已在 resume boundary operand contract 上发布 `wrapper schema -> underlying route`，但 shared surface-resume schema inventory / owner trampoline query 仍未发布 `owner step -> wrapper step` 的反向投影。
- 实现策略调整为：
  - 在 `crates/scoopc/src/effect_lowered/ir.rs` 的 surface-resume dispatch inventory entry 上新增可 stable dump 的 wrapper projection contract；
  - 该 contract 从 resume boundary 已发布的 forward dispatch 中派生，但以独立结构显式发布，避免 `P6-T03` 以后再反向遍历 dispatch plan；
  - 在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` / `layout.rs` 把该 contract 挂到 owner trampoline query，并对缺失、跨 site 漂移或歧义执行 fail-fast；
  - 用 `effect_multi_escape_indirect_direct_while.scoop` 补齐 dump/query/拒绝场景测试。

### 2026-05-04 实现完成

- 已在 `effect_lowered` authoritative surface-resume dispatch inventory 上发布 `wrapper_projection`：
  - 显式包含 `underlying_route`；
  - 显式包含 `owner_step_schema -> wrapper_step_schema`；
  - 显式包含 `complete` 与 `outward_cases` 投影。
- 已在 stable dump 中直接展示该 contract，`dump-effect-lowered` 现在能看到 `k5` 的 `wrapper_projection` 段落。
- 已在 LLVM owner trampoline query 上补齐消费与 fail-fast：
  - 缺失 published projection 时拒绝；
  - derived projection 与 published projection 漂移时拒绝；
  - 多个 resume site 导出不同 projection 时拒绝。
- 已补充测试：
  - late-lowered inventory getter 测试；
  - late-lowered stable dump 测试；
  - LLVM query 正向发布测试；
  - LLVM query 缺失 projection 拒绝测试。
- 已完成验证：
  - `cargo test -p scoopc refactor_surface_resume_dispatch_inventory_ --no-fail-fast`
  - `cargo test -p scoopc refactor_surface_resume_dispatch_dump_exposes_shared_wrapper_projection --no-fail-fast`
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout_ --no-fail-fast`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo fmt --all`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

### 2026-05-04 新一轮执行（P6-T03）

- 本轮先按 `TODO.md` / `TODO-P6-part2.md` 重新定位任务，确认当前第一个未完成详细任务是 `P6-T03`。
- 已确认 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 仍为空壳，而 `llvm/emit.rs` 当前仍对 effectful refactor body 直接 fail-fast；因此本任务不是小修，而是完整 body emitter 落地。
- 已读取 late-lowered IR、LLVM ABI query，并运行真实 fixture 的 `dump-effect-lowered` / `dump-mir` 做 contract 核对。
- 新确认的 blocker：当前 handoff 只发布了 `ResumePayload(boundary, case)` slot identity 与部分 `result_local`，但没有 authoritative 发布“continuation resume 的 incoming payload/answer 应写回哪个 resumed local/home”的统一 contract。
- 若继续硬写 `P6-T03`，backend 将不得不回 canonical MIR 的 `Rvalue::PerformResult` / assign target / paired boundary shape 恢复该绑定，这违反当前阶段“只能消费 published handoff，不得回 shape 恢复语义”的约束。
- 因此本轮计划已切换为：
  - 在 `TODO-P6-part2.md` 中插入最小前置任务 `P6-T02qd`；
  - 同步更新 `TODO.md` 索引与 `P6-T03` 依赖/阻塞记录；
  - 提交这些任务文档与执行记录更新；
  - 停止，等待下一轮按新顺序先完成 `P6-T02qd`。
- 已完成文档同步自检：`TODO-P6-part2.md` 的未完成链路、`P6-T03` 依赖与阻塞记录，以及 `TODO.md` 索引都已更新；当前新的首个未完成详细任务是 `P6-T02qd`。
