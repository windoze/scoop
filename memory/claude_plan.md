# 本轮执行计划（结构化记录）

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行过程中发现已有问题、回归、规格不匹配、未完成实现边界或最新提交提到的遗留问题，则先修复该问题，或在无法直接完成时把它整理为阻塞当前任务的前置任务并更新 `TODO.md` / `PLAN.md`。

## 约束与执行原则

- 不绕过已有问题，不使用临时性 workaround。
- 任何已发现的真实缺陷都视为当前范围内事项。
- 在真正开始实现前，先确认最新提交是否提到需要优先修复的问题。
- 只做一个任务；完成后更新计划与任务状态，运行测试，提交 git commit，然后停止。

## 计划步骤

1. 查看最新一次 git commit，确认是否明确提到待修复的遗留问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 判断该任务是否过大：
   - 若可直接完成，则进入实现。
   - 若过大，则先把它拆分为更小的子任务，更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
4. 在实现前阅读相关代码、测试和规格上下文，确认是否存在阻塞性的既有缺陷。
5. 实现当前任务或优先修复阻塞缺陷。
6. 运行与改动相关的测试；必要时补充或调整测试，直到结果稳定。
7. 更新 `TODO.md`、`PLAN.md`，并同步更新此文件记录关键进展。
8. 检查工作区状态，整理提交内容，创建一次清晰的 git commit。
9. 停止，不继续下一个任务。

## 当前状态

- 已完成：初始化本轮计划文件。
- 已完成：检查最新提交；提交说明未额外声明需要优先修复的新遗留问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认首个未完成叶子任务为 `T4016T2`。
- 已完成：复核当前 `Task` 实现落点（`sysroot/core.scoop`、`runtime/c/scoop_task.c`、HIR lowering、LLVM codegen）。
- 进行中：实现 `T4016T2`。

## 已确认的实现策略

- 不再把 `T4016T2` 继续拆分；本轮直接完成该任务。
- 保留 `T4016T3` 作为“删除遗留 task-only runtime/codegen ABI”的后续任务，本轮不提前删除整套旧 runtime 文件。
- 本轮核心改动方向：
  1. 在可编译 sysroot 源中加入 task 实现文件，把 `Task` 的状态机、`step()`、join/create/from-result 等主体迁到普通 Scoop 代码。
  2. 把私有 step carrier 改成带类型参数的普通 Scoop 定义，使 task state / continuation answer model 与普通类型系统对齐。
  3. 把 async lowering 的内部 helper 落点改到普通 Scoop helper，而不是 `__scoop_task_*` runtime intrinsic。
  4. 取消 `Task.step()` 的 LLVM runtime special-case，让它走普通 Scoop 调用路径；旧 `__scoop_task_*` C ABI 先保留为 `T4016T3` 待删债务。
  5. 更新文档与回归，补一条 LLVM build 级别断言，锁定新路径不再依赖 task-only runtime poll/create ABI。

## 当前执行清单

1. 修改 `sysroot/core.scoop` 的 task 声明面，引入普通 Scoop task state / carrier / helper 声明。
2. 新增 `sysroot/task.scoop`，实现 `Task.step()` 与内部 helper。
3. 调整 sysroot 加载，使 `task.scoop` 作为可编译 sysroot 源参与完整前端/后端流水线。
4. 修改 HIR lowering，把 async sugar 落到新的普通 Scoop helper，并接入带类型参数的 task step carrier。
5. 移除 `Task.step()` 的 LLVM task runtime special-case；保留旧 `__scoop_task_*` ABI 给 `T4016T3` 收尾删除。
6. 更新相关 fixtures / 文档 / 计划文件并跑测试。

## 备注

- 这里记录的是可审计的执行计划与关键决策，不包含原始推理草稿。
## 2026-04-22 续做计划（当前轮）

### 已知状态
- 本轮目标仍然是 `TODO.md` 中首个未完成叶子任务 `T4016T2`。
- 现有未提交改动已经把 task runtime 的大部分路径切到了 ordinary Scoop 实现，但当前被一个真实前置缺口阻塞：
  `sysroot/task.scoop` 中对 `Task.__state` 的赋值在 typecheck 时报 `assignment_target_not_mutable`。
- 该错误不是 task 逻辑本身的问题，而是编译器的成员可变性查询只覆盖当前文件，无法跨文件看到 `sysroot/core.scoop` 中 `Task.__state` 的 `var` 声明。

### 执行计划
1. 先检查当前 worktree 和最新提交信息，确认没有遗漏的预先问题需要先修。
2. 修复 typecheck 阶段的跨文件成员可变性查询：
   - 定位 `member_mutabilities` 的当前来源与使用点。
   - 在 lower/typecheck 共享层增加按成员 FQN 回查声明可变性的能力。
   - 让 assignment mutability 检查在当前文件 map miss 时回退到跨文件查询。
3. 重新构建最小 async/task fixture，确认 `Task.__state` 赋值通过 typecheck，并继续暴露下一个真实问题（如果有）。
4. 若继续遇到 pre-existing issue，则按“先修阻塞问题”的要求继续处理，直到 `T4016T2` 可正确完成；若发现无法在本轮直接修完，则更新 `TODO.md` / `PLAN.md` 重新排依赖并停止。
5. 在 `T4016T2` 主体路径恢复后，完成该任务要求的代码与测试更新，包括：
   - ordinary Scoop `Task.step()` / `__task_*` 路径的实现与调用链验证；
   - 必要的 fixture / LLVM 断言 / 文档调整；
   - 运行相关测试、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
6. 若任务完成：
   - 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`；
   - 提交一个只覆盖本轮完成内容的 git commit；
   - 停止，不进入下一个任务。

### 当前判断
- 目前不应拆分 `T4016T2`，因为阻塞点看起来是一个可以直接修复的编译器缺口，而不是必须先插入 `TODO.md` 的大任务分解。
- 接下来优先级最高的是“跨文件成员可变性查询”，而不是继续扩展 task runtime 代码。

### 进展记录
- 已修改 `crates/scoopc/src/typecheck/expr/collect.rs`：
  - `collect_member_mutabilities(...)` 现在改为收集“当前文件 + env 里其它文件”的成员可变性；
  - 新增 `collect_member_mutabilities_in_file(...)` 作为单文件扫描 helper，避免原逻辑只看当前 AST。
- 已修改 `crates/scoopc/src/typecheck/expr/entry.rs`，让表达式 typecheck 入口把 `env` 传给成员可变性收集。
- 下一步是重新执行最小 async/task build probe，验证 `Task.__state` 赋值的 mutability blocker 是否已消失，并继续处理暴露出的下一个真实问题。

## 2026-04-23 收尾验收计划（当前轮）

### 已确认事实
- `TODO.md` 中首个未完成叶子任务仍是 `T4016T2`。
- 当前 worktree 已经把主要 task 逻辑迁到 `sysroot/task.scoop`，并把 async lowering 的主落点切到 `scoop.core.__task_*` ordinary Scoop helper。
- 本轮已经补掉最后一个阻塞定向回归收尾的旧 emitter 测试：`async_task_resume_replay_ir_terminates_step_fn_on_active_effect` 现在断言真实 IR 不变量，而不是历史 label 名。
- 用户额外要求本轮必须执行“全量测试”，并在确认 `T4016T2` 完成后按 `PROMPT.md` 流程收尾。

### 当前收尾步骤
1. 复核 `T4016T2` 验收口径与当前代码/文档分层是否一致：
   - task driver/state/step 主体是否已在 ordinary Scoop 中；
   - async lowering 是否已改写到 ordinary Scoop helper target；
   - 文档是否已明确 runtime 只保留 continuation/GC/thread/sync substrate 与遗留 ABI 债务边界。
2. 运行全量验证：
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `cargo run -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
3. 若任一命令暴露真实问题，先修复问题再回到验收；不通过 workaround 继续推进。
4. 若全量验证通过且代码/文档满足验收口径：
   - 把 `T4016T2` 标记为完成；
   - 更新 `PLAN.md` 当前阶段状态，让下一未完成任务变成 `T4016T3`；
   - 记录本轮关键验证结论与新增回归覆盖。
5. 整理并提交一次 `[T4016T2] ...` 的收尾 commit，然后停止。

### 收尾结果
- 已确认 `T4016T2` 完成，下一未完成任务变为 `T4016T3`。
- 本轮除 task 主体 Scoop 化之外，还修掉了全量验收暴露的真实前置问题：
  - 跨文件成员 mutability 查询缺口；
  - monomorphized `__task_drive_waiting::<T>` 的 `Continuation.resume(...)` resume-slot rewrite 缺口；
  - cross-package bare enum variant ctor 被 internal helper enum (`__TaskState` / `__TaskStepResult`) 污染的可见性问题；
  - 多个 MIR/typecheck/run-pass golden 与 IR 旧断言的过期收尾项。
- 最终验证结果：
  - `cargo fmt --check`
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/mir` -> `fixtures: ok (6)`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (379)`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass` -> `fixtures: ok (388)`
  - `cargo run -q -p scoop -- test` -> `fixtures: ok (1160)`
  - `cargo test --all -q`
  - `cargo clippy --all-targets -- -D warnings`
- 备注：`scoop test` 过程中仍会打印已有的 warning 级日志（例如 warning-fixture / layout boxing / redundant else 诊断，以及 `UIntPtr` layout field debug warn），但它们未作为 suite 失败项；cargo test 与 clippy 已在当前代码状态下通过。
