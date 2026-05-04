# 当前执行计划

1. 读取 `TODO.md`，确认它只作为索引使用，并记录引用到哪些 `TODO-Px.md` 文件。
2. 按索引顺序检查相关 `TODO-Px.md`，用任务标题是否带有 `[DONE]` 作为唯一完成判定标准，定位第一个未完成的详细任务。
3. 查看最近一次提交信息，判断是否存在与当前任务直接相关且明确标注未完成的问题；若存在，则将其视为当前任务的一部分或作为前置依赖处理。
4. 阅读当前任务的详细要求、约束、验证方式与完成记录，确认是否可以直接完整实现，还是存在必须先补入的真实前置任务。
5. 检查当前工作区状态，识别与当前任务相关的已有未提交修改；如属于同一任务恢复现场，则在本次完成或处理阻塞时一并纳入最终提交。
6. 仅在充分理解当前任务后进行最小必要改动：优先修复根因，不通过缩小范围、替代建模、夹具特判或其他绕行方式前进。
7. 运行与当前任务直接相关的验证：至少包含任务要求的测试；若涉及整体质量门槛，再运行 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或与任务范围等价的必要命令。
8. 若任务完成：
   - 在对应 `TODO-Px.md` 中把任务标题改为 `[DONE] ...`，补充完成记录。
   - 如索引受影响，同步更新 `TODO.md` 中同一任务的 `[DONE]` 标记、标题、顺序或引用。
   - 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
9. 若任务因真实阻塞无法按规格完成：
   - 保持当前任务未完成。
   - 在正确的 `TODO-Px.md` 中插入最小必要的新前置任务，并明确依赖顺序。
   - 同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
10. 在执行过程中，如果计划变化、发现关键阻塞、完成关键实现或验证步骤，及时更新本文件，确保进度可见。
11. 最后按任务编号生成清晰的 git 提交信息，提交当前任务相关全部未提交改动，然后停止，不继续处理下一个任务。

## 说明

- 这里记录的是可审计的执行思路摘要与步骤计划，不包含冗长的内部推理展开。
- 执行原则：一次只完成一个详细任务；若被阻塞，只补前置任务并提交后停止。

## 当前定位结果（2026-05-04）

- 已读取 `TODO.md` 与 `TODO-P6-part2.md`，按“标题是否带 `[DONE]`”规则确认当前第一个未完成详细任务为 `P6-T02q`。
- `TODO.md` 中首个未完成索引项同样是 `P6-T02q`，索引与详细文件在当前任务顺序上保持一致。
- 最近一次提交为 `6e64ff87 [P6-T02q] Add resume wrapper route prerequisite`，与当前任务直接相关，因此其留下的问题视为当前任务上下文的一部分继续处理。
- 当前工作区除本文件外未见其他未提交改动，说明需要基于已提交代码继续把 `P6-T02q` 做完，而不是先收拾一批遗留脏改动。

## 当前执行细化

1. 阅读 `P6-T02q` 关联实现：重点检查 `effect_lowered/ir.rs`、`llvm/codegen/effect_refactor/types.rs`、`layout.rs`，确认当前 resume boundary 只发布 wrapper schema 还是已经部分具备 route bridge。
2. 查找 `effect_multi_escape_indirect_direct_while.scoop` 对应的测试、dump 和 ABI query 使用点，明确缺口落在哪一层：late-lowered handoff、ABI query、还是两者之间的映射。
3. 以最小改动补齐“wrapper schema -> underlying continuation surface route” published contract，并同时加入 fail-fast。
4. 运行任务要求的定向测试与必要的相关测试；若测试揭示还缺少更小但真实的前置依赖，再按规则回写 `TODO-P6-part2.md` / `TODO.md` 后停止。
5. 若实现完成，更新 `TODO-P6-part2.md` 与 `TODO.md` 的 `[DONE]` 标记和完成记录，最后提交一次 git commit 并停止。

## 新发现的阻塞（2026-05-04）

- 在 `effect_multi_escape_indirect_direct_while.scoop` 上跑 `dump-effect-lowered` / `dump-mir` 后确认：`P6-T02q` 所需的 wrapper->underlying route 目前无法仅靠已发布 handoff 唯一恢复。
- 具体根因不是 ABI query 少一层映射这么简单，而是更前面的 continuation write/read provenance 尚未发布：
  - handle arm binder 已发布 `local10 -> continuation_schema=k3`；
  - 但后续 site25/site30/site35/site40 的 resume boundary continuation local（`local95` / `local116` / `local137` / `local158`）来自 `MemberAccess(Cell.k)` + `PatternExtract(Some[0])`；
  - 与此同时，`crates/scoopc/src/mir/lower.rs::lower_assign_stmt(...)` 仍只支持 `local = expr`，`cell.k = Some(k)` / `cell.k = none_k` 会落成 `StatementKind::Todo("assign lhs lowering pending")`；
  - 这意味着当前 canonical MIR / late-lowered handoff 根本没有显式发布“handle binder continuation 经 `cell.k` 写回、再读回到 resume local”的 authoritative provenance contract。
- 结论：若继续硬做 `P6-T02q`，实现将不可避免地回 unresolved assign-lhs TODO、source shape、member 名字或 continuation nominal type 去猜 route，违反用户要求与当前阶段 contract-first 边界。

## 本次计划调整

1. 不继续实现 `P6-T02q` 本体。
2. 在 `TODO-P6-part2.md` 中新增最小前置任务 `P6-T02qa`，要求先发布 escaped continuation aggregate/member write-read provenance contract。
3. 同步更新 `TODO.md` 索引，并把 `P6-T02q` 依赖改为显式依赖 `P6-T02qa`。
4. 不更新 `PLAN.md`，因为阶段级计划未变，只是细化了当前详细执行顺序。
5. 提交本次任务拆分与阻塞记录后停止，等待下一次 invocation 先完成 `P6-T02qa`。
