# 当前执行计划

## 约束说明
- 不输出或记录逐字的私有推理细节；这里记录可审阅的任务计划、决策依据、关键发现和执行进展。
- 本次目标是：先定位 `TODO.md` 对应详细任务列表中的首个未完成任务，完成该任务或在遇到真实阻塞时补充最小前置任务，然后更新任务文档、验证、提交并停止。

## 初始步骤计划
1. 读取 `TODO.md`，确认它作为索引引用了哪些 `TODO-Px.md` 文件。
2. 按任务顺序检查对应 `TODO-Px.md` 中的标题状态；只有标题前缀为 `[DONE]` 才视为完成。
3. 定位首个未完成详细任务，并检查最新提交是否直接提到与该任务相关且未完成的问题。
4. 阅读该任务的详细要求、约束、依赖、验收方式和完成记录。
5. 结合当前代码状态做最小范围实现；若发现阻塞当前任务的真实缺口或回归，则先修复，或在必要时新增最小前置任务并同步 `TODO.md`。
6. 运行与该任务直接相关的测试，再运行必要的整体检查（至少包含本任务相关测试，若改动影响较广则补充更高层验证）。
7. 更新对应 `TODO-Px.md`：若任务完成，则将标题标记为 `[DONE]` 并填写完成记录；若发生阻塞，则记录阻塞和新增前置任务。
8. 若任务索引信息发生变化，同步更新 `TODO.md`；仅在阶段计划发生真实变化时更新 `PLAN.md`。
9. 检查工作区状态，按要求创建一次清晰的 git 提交，然后停止，不继续做下一个任务。

## 进展记录
- 已创建本计划文件，待开始读取任务索引并定位当前应执行的首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认首个未完成详细任务为 `P4-T02a：修复 canonical materialized MIR pass-view 对普通非泛型 callable body 的发布`；`P4-T03` 仍因该前置问题保持未完成。
- 已检查最新提交 `62802c47 [P4-T02a] Track ordinary callable pass-view prerequisite`，其主题与当前任务直接相关，说明本轮应继续完成同一任务而不是切到别的条目。
- 已检查当前未提交改动，涉及 `mir/materialize.rs`、`mir/pass_view.rs`、`mir/inline.rs`、`llvm/tests.rs`、`effect_refactor_pipeline/effect_facts_stage.rs` 以及 `effect_facts/builder.rs`；从 diff 看，核心方向是把 ordinary non-generic callable 正式发布到 canonical `pass_view`，并补充对应断言与测试。
- 已做一处最小一致性修正：把 `crates/scoopc/src/mir/inline.rs` 中仍沿用旧语义的测试名改为匹配当前设计的描述，避免测试名继续暗示“只有被改写的 non-generic body 才进入 pass-view”。
- 额外 LLVM 回归首轮失败，定位到失败点是测试本身把 `O2` 后 `caller` 必须仍保留为独立 IR 函数当成硬约束；这与本任务真正需要验证的“production codegen 能消费 canonical pass-view 发布的 body”不是同一件事。已将该测试改为：优先检查 `caller`，若其已被 LLVM 继续内联，则退回检查 `main`，从而稳定验证最终 IR 不会重新调用 `wrap` / `id`。
- `cargo clippy` 过程中还暴露出一个与本轮代码无关但会阻塞“提交无告警”目标的既有 lint：`effect_refactor_pipeline::emit_production_llvm_artifact_to_file` 参数个数触发 `clippy::too_many_arguments`。该仓库已有同类局部 `allow` 用法，因此已按既有风格补上局部 `#[allow(clippy::too_many_arguments)]`，避免为 lint 扩大本轮功能改动面。
- 下一步：逐个核对这些改动是否真正让 `pass_view().instances()`、`owner_of_callable()`、`root_body()`、`callable_bodies()` 对 ordinary non-generic callable 生效；若实现已齐备，则直接运行任务要求的定向测试与必要的质量检查，再回写 `TODO-P4.md` / `TODO.md` / `memory/claude_plan.md` 并提交。
- 已完成任务文档回写：`TODO-P4.md` 中 `P4-T02a` 已标记为 `[DONE]`，`TODO.md` 索引也已同步；`P4-T02aR` 仍保持未完成，供下一轮单独 review。
- 已完成验证：定向 `pass_view` / `effect_facts_stage` / `mir::inline` / LLVM regression 均通过，`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings` 与 `cargo clippy -p scoopc --all-targets -- -D warnings` 也已通过。
- 剩余步骤：检查最终 worktree，按 `P4-T02a` 创建一次提交，然后停止。
