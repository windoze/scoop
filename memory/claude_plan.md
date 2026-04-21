# 执行计划与决策摘要

说明：基于安全与协作边界，这里不记录逐字内部推理，而是记录可审阅的决策摘要、执行计划、关键假设与进度更新。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果遇到阻塞，则按要求重排 `TODO.md` / `PLAN.md`，提交后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交，确认是否提到需要优先修复的既有问题。
2. 检查当前工作树状态，识别是否存在用户未提交修改，避免误覆盖。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖和既有拆分。
5. 判断该任务是否可在本轮完整完成：
   - 若可完成，直接实现。
   - 若过大或存在前置缺口，先把任务拆分/重排到 `TODO.md` 与 `PLAN.md`，本轮只执行拆分后的首个子任务或记录阻塞。
6. 实现任务并补充/调整测试。
7. 运行相关验证，至少覆盖：
   - 任务相关测试
   - 必要时运行更大范围回归
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
8. 更新文档状态：
   - 在 `TODO.md` 标记该任务完成或按依赖重排
   - 在 `PLAN.md` 记录当前状态与后续顺序
   - 在本文件记录关键进度
9. 使用清晰提交信息提交本轮变更，然后停止。

## 当前假设

- 仓库可能存在未提交修改，必须避免回退非本轮变更。
- “最新提交中的既有问题” 只有在提交信息或代码状态明确指向时才需要优先修复。
- 只有在确认第一个未完成任务后，才能决定是否需要进一步拆分。

## 进度记录

- 已创建本计划文件，准备开始仓库检查。
- 已检查 `git log -1`、`TODO.md`、`PLAN.md`。
- 最新提交 `2fb8af7 [T4016d] Thin Task around shared continuation helper` 未在提交说明中显式声明新的既有问题需要先修。
- 当前首个未完成条目为 `T4016R`，其性质是 review 任务：需要核对 continuation / `Task` 的生产实现、文档叙事与测试覆盖是否一致。
- 下一步：
  1. 搜索仓库中是否仍残留用户态 `-> resume`、旧 `ImmediateResume` 语义或 task-only runtime hack 叙事。
  2. 审查 `Task` / continuation 相关实现与运行时 helper，确认 `Task` 是否仅作为 continuation thin wrapper。
  3. 复核与 `T4016R` 直接相关的测试，再决定是收口该 review，还是前置新增缺陷任务。
- 已完成静态审查：
  - `-> resume` 仅剩 removed-syntax parser diagnostic；
  - `Continuation.resume(...)` 的 typecheck / lowering / LLVM codegen 已统一走 answer-returning helper；
  - `Task` 仅在私有层把同一 continuation answer 通道解释为 `__TaskStepResult`，未见继续偷读 continuation frame 前缀的路径。
- 已完成定向验证：
  - `cargo test -p scoop_runtime --test continuation_one_shot --test task_spawn_join` 通过；
  - `cargo test -p scoop_runtime --test gc_enter_native` 通过。
- 在全量 `cargo run -p scoop -- test` 中发现新的 pre-existing blocker：
  - 失败 fixture：`tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`
  - 现象：期望 `SCOOP_GC_MOVE=1` 下通过，实际 `exit(3)`。
  - 复现：单独 build 成功，但手动运行 `SCOOP_GC_MOVE=1 /tmp/extern_enter_native_roots_gc.out` 仍 `exit(3)`。
- 阻塞问题的当前判断：
  - 该 fixture 的 LLVM IR 已生成 `scoop_enter_native(root_slots = 1)`，说明“完全没插入 native roots”不是问题本体；
  - 但 extern body statepoint 返回后，IR 继续沿用 native 期间的 SSA `gc.relocate` 值，并在 `scoop_leave_native()` 之后写回局部 `%x`；
  - moving GC 若已通过 `native_roots` 把 `%x` 更新到新地址，此写回会把 stale/pre-move 指针重新写回 managed frame，导致后续 `GC.handleNew/handleGet` 路径失败。
- 计划已变更：
  1. 不继续尝试完成 `T4016R`。
  2. 先在 `TODO.md` / `PLAN.md` 中新增前置任务 `T1510c1`，明确该 `@Extern` + moving-GC native-roots 回归。
  3. 让 `T4016R` 显式依赖 `T1510c1`，本轮只提交任务重排与阻塞说明后停止。
- 已完成重排：
  - `TODO.md` 已新增 `T1510c1 [TODO]`，放在 `T4016d` 与 `T4016R` 之间；
  - `T4016R` 已改为依赖 `T1510c1, T4016d`；
  - `PLAN.md` 已补记该 blocking mismatch、IR 证据与新的执行顺序 `T1510c1 -> T4016R -> ...`。
