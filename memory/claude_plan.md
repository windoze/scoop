# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务，完成后更新计划与任务状态、执行验证、提交 Git commit，然后停止。

## 约束与执行原则

1. 在开始任何实现前，先检查最新一次提交是否提到已知遗留问题；如果提到，则该问题优先，必须先修复。
2. 任何在检查、测试、实现过程中发现的既有缺陷、回归、规范不匹配、实现边界缺失或依赖缺口，都视为当前范围内问题：
   - 若能直接修复，则先修复，再继续当前任务。
   - 若无法在本轮直接完成，则必须把它作为前置任务写入 `TODO.md`，调整依赖顺序，更新 `PLAN.md`，提交后停止。
3. 不允许通过规避路径、缩小规格、替换表示、削弱测试或引入临时特判来“绕过”问题。
4. 本轮必须记录关键进展；如果计划发生变化，或完成了关键步骤，我会继续更新本文件。

## 初始步骤

1. 查看最新一次 Git commit，确认是否显式提到待修复的遗留问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有任务拆分、依赖关系和上下文。
4. 判断该任务是否过大：
   - 如果过大，则拆分为更小的可执行子任务，并同步更新 `PLAN.md` / `TODO.md`。
   - 如果足够具体，则直接进入实现。
5. 结合任务范围检查相关代码、测试、规格和现有实现。
6. 实现任务，并在过程中修复所有阻塞性的既有问题。
7. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响面较大，补充更高层级验证。
   - 在可行范围内检查格式、编译、测试与 lint，无新增 warning。
8. 更新文档状态：
   - 在 `TODO.md` 中标记该任务完成，或在受阻时按依赖顺序重排任务。
   - 在 `PLAN.md` 中记录完成情况、设计调整和依赖变化。
   - 在本文件中补充本轮执行结果与关键决策。
9. 使用清晰的 commit message 提交本轮变更。
10. 停止，不进入下一个任务。

## 需要重点核查的内容

- 最新提交描述中是否存在“已知但未修”的问题。
- 当前首个未完成任务是否依赖尚未实现的语言特性、运行时能力或编译器行为。
- 改动是否引入 workaround 风险或掩盖既有规范不匹配。
- 是否存在必须同步更新的文档、测试或夹具。

## 进度记录规则

- 完成“最新提交检查”后更新本文件。
- 确认首个未完成任务并评估是否需要拆分后更新本文件。
- 开始实现前记录实现范围。
- 完成测试后记录验证结果。
- 提交前记录最终结论与提交说明。

## 当前进度

### 2026-04-22 初始检查结果

- 已检查最新提交：`a82ec6e1 [T4016T1d4] Align single-file LLVM frontend with build`。
- 该提交的 commit message 未显式提到新的遗留问题或“提交后仍待修”的事项，因此目前没有来自最新提交描述、必须先于 `TODO.md` 处理的额外问题。
- 已读取 `TODO.md` / `PLAN.md` 并确认当前顺序。
- 当前首个未完成任务为：`T4016T1d5`，主题是“为 ordinary Scoop task 持有的 sync 资源补齐无泄漏释放合同”。

### 下一步

1. 详细阅读 `T4016T1d5` 的任务说明、依赖和验收条件。
2. 检查现有 `Task` / `Mutex` / sync runtime / sysroot 相关实现，确认当前泄漏点与已有销毁合同。
3. 判断 `T4016T1d5` 是否可以直接在本轮完成；若范围仍然过大，则先细化到新的前置子任务并更新 `TODO.md` / `PLAN.md`。

### 2026-04-22 任务评估结果

- `T4016T1d5` 的范围已足够具体，本轮可以直接实现，不需要再拆新的前置子任务。
- 已确认当前 blocker 形状：
  - 未来 ordinary Scoop `Task` 会像 `tests/fixtures/run-pass/task_generic_state_object_model_basic.scoop` 那样把 `Mutex` 作为普通字段持有；
  - 该 `Mutex` 本身是独立的 GC 对象，因此只要 sync 对象自己在 sweep 前执行 `release_fn`，包含它的普通 Scoop 对象在丢弃后就不会泄漏平台资源；
  - 现状中 `runtime/c/scoop_task.c` 已通过 `release_fn` 自动销毁内嵌 task lock，但 `runtime/c/scoop_sync.c` 仍停留在“只靠显式 destroy()，未来再接 finalizer”的旧实现；
  - 进一步检查发现同类问题不只影响 `Mutex/CondVar`，`Once` 也内部持有平台 `Mutex + CondVar`，当前同样缺少 GC sweep 前的释放路径。

### 当前实现计划

1. 在 `runtime/c/scoop_sync.c` 中把 `Mutex`、`CondVar`、`Once` 改为 typed allocation，并为它们补齐 `ScoopTypeDescriptor.release_fn`。
2. 把显式 `destroy()` 与 GC sweep release 收口到同一组内部 helper，确保：
   - 显式 `destroy()` 仍然可用，并作为“提前释放平台资源”的路径；
   - sweep release 在对象不可达时自动清理未显式释放的资源；
   - 显式 `destroy()` 后再次被 GC sweep 不会 double-destroy。
3. 增加最小测试观测面：
   - 为测试导出 sync destroy 计数器 reset / getter；
   - 新增 run-pass regression，验证普通 Scoop object 持有的 `Mutex/CondVar/Once` 在对象丢弃并 GC 后会释放资源，且显式 `destroy()` 不会被 sweep 重复释放。
4. 视需要同步更新 `sysroot/sync.scoop` 注释，使文档反映“显式 destroy + GC release cleanup”的现状。

### 2026-04-22 已完成实现

- 已在 `runtime/c/scoop_sync.c` 中完成：
  - `Mutex` / `CondVar` / `Once` 全部改为 `scoop_alloc_typed(...)` 分配；
  - 为三类 sync 对象补齐 `ScoopTypeDescriptor.release_fn`；
  - 显式 `destroy()` 与 GC sweep cleanup 已统一走内部 `*_destroy_impl(...)` helper；
  - `Mutex` / `CondVar` 新增 `initialized` 防护，避免 typed allocation 失败后 future sweep 误销毁未初始化平台对象；
  - `Once` 新增初始化 flag，确保内部 `lock/condvar` 在 create 失败与 sweep cleanup 两条路径上都不会重复销毁。
- 已新增测试观测接口：
  - `scoop_test_sync_destroy_counts_reset`
  - `scoop_test_sync_mutex_destroy_count`
  - `scoop_test_sync_condvar_destroy_count`
  - `scoop_test_sync_once_destroy_count`
- 已更新 `runtime/c/scoop_runtime_api.h` allowlist，避免新增导出符号触发 ABI 审计失败。
- 已更新 `sysroot/sync.scoop` 注释，使其与当前合同一致：显式 `destroy()` 仍是提前释放路径，但不可达对象会在 runtime sweep 前做受限 cleanup。
- 已新增 run-pass 回归 `tests/fixtures/run-pass/sync_gc_release_task_like_object_basic.scoop`，用普通 Scoop object 字段直接覆盖：
  - 丢弃后 GC cleanup；
  - 显式 destroy 的提前释放；
  - sweep 不会 double-destroy。

### 当前测试状态

- `cargo fmt` 已通过。
- `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已通过：`fixtures: ok (388)`。
- `cargo run -p scoop -- test` 已通过：`fixtures: ok (1160)`。
- `cargo test --all` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。

### 2026-04-22 收尾状态

- `TODO.md` 已将 `T4016T1d5` 标记为 `[DONE]`，并记录完成内容与验证命令。
- `PLAN.md` 已同步更新当前阶段状态，后续首个未完成任务为 `T4016T2`。
- 本轮未再发现需要先于 `T4016T2` 插入的新 blocker；下一次调用应从 `T4016T2` 开始。
- 待执行的最后一步：提交本轮改动并停止。
