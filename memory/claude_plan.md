# 执行计划

说明：按要求先记录可公开的思路摘要与执行步骤。这里不写内部逐字推理，而是记录可核查的判断依据、计划、进度和后续调整。

## 当前目标

本轮只处理 `TODO.md` 中“第一个未完成任务”，但在此之前必须先检查最新提交是否提到已有问题；若提到，则先修复该问题。执行过程中如果发现任何既有 bug、回归、规范不匹配、实现边界缺口或临时绕过，也必须立即纳入当前范围，优先修复或在 `TODO.md` 中作为前置任务插入。

## 执行顺序

1. 检查最新一次 Git 提交信息与改动，确认是否显式提到待修问题。
2. 打开 `TODO.md`，定位第一个未完成任务。
3. 打开 `PLAN.md`，核对现有计划与任务顺序是否一致。
4. 评估该任务复杂度：
   - 如果任务足够明确且可在本轮完整交付，直接实现。
   - 如果任务过大或依赖缺失，则先把任务拆分并更新 `PLAN.md` / `TODO.md`，本轮执行拆分后的第一个子任务。
5. 实现任务时同步检查相关代码、测试和规范边界，识别是否存在阻塞性的既有问题。
6. 运行与改动直接相关的测试；在合适范围内补充更广的验证，包括至少：
   - 相关定向测试
   - `cargo fmt --check` 或必要时 `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - 视改动范围决定是否运行更广的 `cargo test --all` 或相关子集
7. 更新文档与计划：
   - 在 `TODO.md` 中将完成任务标记为完成，或在阻塞时调整顺序并补充前置任务
   - 在 `PLAN.md` 中记录当前状态、拆分结果、阻塞关系或完成情况
   - 本文件追加进度记录与计划变更
8. 提交 Git commit，提交信息应清晰描述本轮完成内容。
9. 停止，不继续下一个任务。

## 约束与决策原则

- 不接受规避式修复，不通过缩小范围、修改夹具形状、走更窄路径来“绕过”缺陷。
- 发现既有问题时，优先级高于当前计划任务。
- 若任务被前置缺陷阻塞，必须先在 `TODO.md` 中把修复缺陷的任务插到前面，并在 `PLAN.md` 与此文件中说明原因，然后提交并停止。
- 不回退用户已有修改；若工作树脏，必须基于现状小心增量修改。

## 进度记录

- 已完成：创建本文件并记录初始执行计划。
- 已完成：检查最新提交 `339b5a5fbd8039a16354c244f0ee6fc24e15070f`；提交说明是已完成修复 `[T4016T5a] Fix UTF-8 source span panic in ctor codegen`，未额外挂出新的待修问题，因此无需先插入额外修复。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T4016T6`：把 core `Task` object model 从 per-task `Mutex` 改成轻量 atomic claim field。
- 当前判断：`T4016T5` 已先行补齐 object-field atomic 能力，`T4016T6` 看起来可以直接实施；接下来需要核实现有 `Task` 布局、`step()` 过渡实现以及原子字段/原子 helper 的可用形状，确认是否仍有隐藏 blocker。
- 已完成：审阅 `sysroot/core.scoop`、`sysroot/task.scoop` 与 `scoop.unsafe.__AtomicInt` / `__atomicInt*` 接口；确认可以在不前做 `T4016T7` trap 语义的前提下，先把 task 对象模型与内部短临界区切到 atomic claim。
- 已完成：实现 `T4016T6`。
  - `Task<T>` 内部字段已从 `__lock: scoop.sync.Mutex` 切到 `__claim: scoop.unsafe.__AtomicInt`。
  - `sysroot/task.scoop` 已删除 `mutexCreate()` / `lock()` / `unlock()`，改为 `__task_claim_acquire()` / `__task_claim_release()` + `__atomicIntCompareExchange` / `__atomicIntStore`。
  - `Task.step()` 及相关 helper 现已使用 atomic claim helper 串行化状态迁移，同时保留 `Running -> Pending` 的过渡语义，把 trap-on-contention 留给后续 `T4016T7`。
  - 新增 LLVM fixture `tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop`，锁定 task 主线不再生成 `scoop_sync_mutex_*` 调用且会发出 atomic 指令。
- 已完成：修复实施后暴露的连带回归。
  - 初次跑 build fixtures 时，我错误移除了 `sysroot/task.scoop` 对 `scoop.thread.*` 的 import，导致 `yield()` 无法解析；已补回该 import。
  - 全量 fixtures 首次运行时，`tests/fixtures/mir/closure_capture_val.scoop` 与 `tests/fixtures/mir/closure_capture_var.scoop` 的 MIR golden 因 sysroot 类型表重排而只发生 `TypeId` 编号前移；确认结构未变后，已同步更新两份 golden。
- 已完成：验证。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build` 通过（`fixtures: ok (17)`）。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 通过（`fixtures: ok (389)`）。
  - `cargo run -p scoop -- test` 通过（`fixtures: ok (1163)`）。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 下一步：更新 `TODO.md` / `PLAN.md` 与本文件，提交本轮变更，然后停止。
