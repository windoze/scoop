# 本次执行计划

说明：按要求先记录执行计划与进度更新点。出于推理隐私与安全原因，这里不写逐字内部思考，只记录可审计的执行步骤、判断依据、风险点与阶段性结论。

## 目标

完成 `TODO.md` 索引所指向的第一个“详细任务文件中尚未标记为 `[DONE]`”的任务；若出现阻塞，则按要求补充最小前置任务、同步索引并提交后停止。

## 执行步骤

1. 读取 `TODO.md`，确认它只是索引，并找出引用的 `TODO-Px.md` 文件及顺序。
2. 依序检查相关 `TODO-Px.md` 文件，定位第一个标题未带 `[DONE]` 的详细任务。
3. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若有，则纳入当前任务范围或作为前置依赖处理。
4. 阅读当前任务的详细要求、约束、依赖、验证标准与完成记录。
5. 检查工作区当前状态，避免覆盖他人未提交改动；仅在与当前任务相关的文件上进行最小修改。
6. 实施任务所需代码/文档/测试改动。
7. 运行与任务直接相关的验证：至少包含任务要求的测试；若改动影响面较广，再补充必要的 `cargo test` / `cargo clippy --all-targets -- -D warnings` / 相关夹具测试。
8. 若验证失败，先修复失败项，再重新验证。
9. 更新详细任务文件：将当前任务标题前缀改为 `[DONE]`，补全完成记录。
10. 若任务编号、标题、顺序或完成状态影响索引，则同步更新 `TODO.md`。
11. 仅在阶段计划/依赖结构真实变化时更新 `PLAN.md`；否则不改。
12. 复查改动集，确认没有引入与任务无关的修改。
13. 按仓库约定创建一次 git 提交，然后停止，不继续做下一个任务。

## 阻塞处理规则

- 若发现当前任务依赖尚未跟踪的具体前置条件：
  - 在对应 `TODO-Px.md` 中插入最小必要前置任务；
  - 明确当前任务对该前置任务的依赖；
  - 同步更新 `TODO.md`；
  - 仅当阶段计划受影响时更新 `PLAN.md`；
  - 提交并停止。
- 不以缩小范围、改换表示、夹具规避或临时特判方式绕过规范缺口。

## 进度记录

- 初始状态：已写入本计划，尚未读取任务索引。
- 已完成：读取 `TODO.md` 与 `TODO-P6.md`，确认首个未完成详细任务为 `P6-T02e`。
- 已完成：检查最近一次提交，`[P6-T02e] Track pure caller runtime-error blocker` 直接对应当前任务，说明本轮无需先做开放式排障，而应先补该 blocker 的 authoritative handoff。
- 当前判断：现有 `LateLoweredCallBoundaryLowering::consumed_runtime_error_case` 只记录输入 case 身份，尚未发布 backend 可执行的 caller-local 路径，因此需要补结构化 lowering contract，而不是继续把语义留给 P6 backend 猜测。
- 选定实现方向：
  1. 扩展 `consumed_runtime_error_case`，让它显式携带 caller-local target state，而不再只是“记住 case id”；
  2. 在 late-lowered `state_graph` 中为每个 pure caller call boundary 的本地 runtime-error 路径新增 synthetic terminal state，并把它挂回 owner `Suspend` state 的显式后继集合；
  3. 新增对应的 state terminator 表达该路径是 caller-local runtime-error terminal path；
  4. 在 refactor LLVM ABI/query 层发布可验证的查询面，保证 P6-T03 以后只能消费这份 contract；
  5. 补充 dump / 单测 / fail-fast 断言，再跑任务要求的验证命令。
- 已完成实现：
  - `LateLoweredConsumedRuntimeErrorCase` 已扩展为显式携带 `target_state`；
  - `LateLoweredStateTerminator::Suspend` 已新增 `local_runtime_error_states`；
  - late-lowered state graph 已为 pure caller call boundary 自动追加 synthetic `LocalRuntimeError` terminal state；
  - refactor LLVM ABI query 已新增 local runtime-error contract 发布与 fail-fast 校验；
  - dump / 单测 / query 测试已同步覆盖。
- 已完成验证：
  - `cargo test -p scoopc refactor_boundary_lowering`
  - `cargo test -p scoopc refactor_effect_lowered_stage`
  - `cargo test -p scoopc refactor_llvm_local_runtime_error_contract`
  - `cargo run -q -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 当前待收尾：
  - 更新 `TODO-P6.md` / `TODO.md` 完成记录；
  - 检查 diff 后创建本次提交并停止。
- 收尾进度：`TODO-P6.md` 与 `TODO.md` 已同步标记 `[DONE]`；当前仅剩 git 提交。
- 后续会在以下节点更新本文件：
  - 确认首个未完成任务后；
  - 开始实施改动前；
  - 完成主要实现后；
  - 完成测试验证后；
  - 提交前记录最终结果。
