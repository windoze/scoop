# 本轮执行计划（公开版）

## 约束说明

用户要求先把计划写入本文件，再执行任何命令。这里记录的是可公开的执行计划、检查步骤和关键决策，不包含模型的私有推理细节。

## 初始计划

1. 查看最近一次提交，确认是否明确提到需要先修复的既有问题；若有，优先处理。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和是否需要拆分。
4. 检查工作区状态，避免覆盖用户已有修改。
5. 如第一个未完成任务过大，先把它拆成更小子任务，并同步更新 `TODO.md` / `PLAN.md`，然后只执行新的第一个子任务。
6. 实现当前目标任务，同时对过程中暴露出的既有缺陷做排查；若发现阻塞性既有问题，先修复，或按要求把它插入 `TODO.md` 作为前置任务并停止。
7. 运行相关格式化、测试、`clippy`，确保无警告、无回归。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态或阻塞关系调整。
9. 提交本轮修改，提交后停止，不进入下一个任务。

## 预期检查清单

- 最近提交信息是否声明了待修复问题
- `TODO.md` 第一个未完成项是否足够具体、可在一轮内完成
- 当前任务是否依赖缺失特性或既有缺陷修复
- 实现后相关测试是否全部通过
- `cargo clippy --all-targets -- -D warnings` 是否通过
- 文档与任务状态文件是否已同步

## 执行中更新区

- 已写入初始计划，尚未开始读取仓库状态。
- 已检查最近提交：`[T4016T1d2] Fix generic helper/member monomorph type-param leaks`，提交正文未额外声明尚未修复的既有问题。
- 更正：工作区并非只有本文件改动；上一轮未提交的 `T4016T2` 试探性实现仍在，涉及 `sysroot/task.scoop`、`task_api.scoop`、HIR lowering、sysroot loader 与部分 LLVM task-only 路径删除。
- 已定位当前第一个未完成的叶子任务：`T4016T2`。
- 下一步：补读 `T4016T2` 对应的文档与实现，判断是否需要继续拆分。
- 已确认 `T4016T2` 的主要现状：
  - `async` lowering 仍指向 `__scoop_task_create` / `__scoop_task_step_ready` / `__scoop_task_step_pending`。
  - LLVM codegen 仍把 `step()` 与 `__scoop_task_*` helper 直接 special-case 到 `scoop_task_*` runtime ABI。
  - `runtime/c/scoop_task.c` 仍持有真实 task object / state / step-driving 逻辑。
- 新发现的实现风险：
  - 若按设计把 `Task` 改为持有普通 `scoop.sync.Mutex`，当前 `Mutex` 运行时对象默认依赖显式 `destroy()`，没有 GC finalizer；这会让 task 内部锁系统性泄漏。
  - 该问题不能通过“继续沿用 task-only C struct”规避；需要在本轮实现中一并收口，或在确认不可控后把它作为前置任务写回 `TODO.md`。
## 2026-04-22 本轮续作计划

### 已知前情
- 上一轮已确认最近提交 `[T4016T1d2] Fix generic helper/member monomorph type-param leaks` 没有额外声明待修复问题。
- 当前尝试中的首个未完成叶子任务是 `T4016T2`，目标是把 task 主体迁回普通 Scoop 实现。
- 现有未提交改动已经把 `task.scoop` / `task_api.scoop`、HIR lowering 的 `__TaskStepResult<T>` 泛型化、以及部分 LLVM task-only 直连逻辑删除推进到一半，但仍未稳定。
- 已暴露两个关键阻塞：
  1. `sysroot/task.scoop` 中对 enum variant 的解析/构造不稳定，当前报 `未解析的成员：scoop.core.TaskStep.Ready`。
  2. `emit_minimal_main_ir(...)` 这条最小 IR 路径高概率没有把可编译 sysroot 源文件纳入，导致 `UnsupportedMainBody { kind: "call callee type" }`。
- 还存在一个风险：若普通 Scoop `Task` 直接持有 `Mutex`，当前 runtime 的释放模型可能导致底层平台 mutex 资源泄漏；需要判断它是否已经构成 `T4016T2` 的真实前置阻塞。

### 本轮执行计划
1. 先核对当前工作树、`TODO.md`、`PLAN.md` 与 `memory/claude_plan.md`，确认第一未完成任务及其依赖没有被上一轮遗漏。
2. 最小化复现并定位 enum variant 解析问题，重点确认：
   - 签名文件定义 enum、另一个可编译文件中裸 variant 名是否可解析。
   - `import scoop.core.*` 是否会把 variant 引入值命名空间。
   - 限定写法 `Type.Variant(...)` / `Type.Variant` 在表达式和 pattern 位置各自的支持边界。
3. 如果这是仓库既有前端缺口且不能在当前任务内直接小步收口，就按要求把 blocker 插成 `TODO.md` 中位于 `T4016T2` 之前的新前置任务，同步更新 `PLAN.md` 与本文件，然后提交并停止。
4. 如果 enum variant 问题可以直接修复，就继续把 `sysroot/task.scoop` 跑通，再修 `emit_minimal_main_ir` 对可编译 sysroot 源的接入，使 `T4016T2` 能完整落地。
5. 在实现过程中持续更新本文件，记录关键发现、计划调整、完成的验证，以及任何新暴露的既有问题。

### 当前判定标准
- 不接受通过改写 task 表示形状、缩窄 fixture、保留 task-only 特判等方式绕过 blocker。
- 若发现普通 Scoop `Task` 的锁释放模型确实不满足现有 runtime 约束，则必须把“修正资源释放语义”作为前置任务处理，而不是带着泄漏继续推进。

## 本轮关键发现（2026-04-22）

- 已复现 `T4016T2` 当前直接失败：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/task_step_manual_basic.scoop -o /tmp/task_step_manual_basic.out`
  - 结果：`scoop::resolve::unresolved_member`，错误为 `未解析的成员：scoop.core.TaskStep.Ready`。
- 已复现最小 IR 路径失败：
  - `cargo test -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - 结果：`UnsupportedMainBody { kind: "call callee type" }`。
- 已最小化确认限定 enum variant 的两个独立缺口：
  - 表达式位置的 payload ctor `Enum.Variant(...)` 还未走通；当前 task sysroot 里 `TaskStep.Ready(value)` 会直接卡在 resolver。
  - `when` pattern 的限定写法 `Enum.Variant(...)` 仍无 parser 支持；最小 probe 报“期望 `->`，但遇到 `.`”。
- 已确认第三个真实 blocker：
  - `sysroot/sync.scoop` 与 `runtime/c/scoop_sync.c` 仍把 `Mutex`/`CondVar` 明确成“显式 `destroy()` 释放平台资源”的模型；
  - `runtime/c/scoop_task.c` 现有 task-only 实现通过 typed object `release_fn` 销毁内部 mutex；
  - 若 ordinary Scoop `Task` 直接持有 `Mutex` 而没有新的 release path，将系统性泄漏底层 mutex 资源。

## 计划调整

根据以上确认，本轮不继续硬推 `T4016T2`。改为：

1. 把 blocker 前插成 `TODO.md` / `PLAN.md` 中位于 `T4016T2` 之前的新任务：
   - `T4016T1d3`：限定 enum variant ctor / `when` pattern 主线；
   - `T4016T1d4`：single-file / minimal IR 路径纳入可编译 sysroot 源；
   - `T4016T1d5`：ordinary Scoop task 的 sync 资源释放合同。
2. 回退上一轮留下的半成品 `T4016T2` 代码改动，避免把失败中的实现一并带入 blocker 提交。
3. 自检 `TODO.md` / `PLAN.md` / 本文件的顺序与说明，然后只提交“任务重排 + blocker 记录”，本轮停止。
