# Claude Plan

## 约束说明
- 按要求先记录执行计划，再进行仓库检查与任务实现。
- 这里记录的是可审阅的步骤摘要、假设与决策，不包含完整内部推理细节。

## 初始执行计划
1. 检查最新一次 Git 提交的提交信息与变更，确认是否提到需要先修复的既有问题；若有，优先修复并验证。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务规模与依赖：
   - 若可直接完成，则继续实现。
   - 若过大或被前置缺陷阻塞，则分解任务并更新 `PLAN.md` 与 `TODO.md`，使第一个子任务成为当前执行目标。
4. 阅读与当前任务相关的代码、规范、测试和计划文件，确认实现边界与现状。
5. 实现当前目标任务，必要时补充/调整测试与文档。
6. 运行必要验证，至少覆盖：
   - 相关测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务相关，再运行额外 fixture/spec 检查
7. 根据执行结果更新文档：
   - 在 `TODO.md` 中标记完成，或在阻塞时按依赖顺序重排任务
   - 在 `PLAN.md` 中记录当前状态、分解、阻塞原因或后续安排
   - 在本文件中同步关键进展与计划变化
8. 检查工作区改动，确保不误改无关内容。
9. 使用清晰的提交信息提交本次完成的单个任务，然后停止。

## 风险与决策准则
- 若发现规范不一致、缺失语言特性、运行时缺陷或测试仅靠变通才能通过，则视为真实项目问题；必须先在 `TODO.md`/`PLAN.md` 中建前置任务并调整顺序，不能绕过。
- 不回退或覆盖非本次任务引入的现有修改；若遇到冲突，先分析是否阻塞当前任务。
- 每完成关键里程碑（定位任务、决定分解、完成实现、完成验证、准备提交）后，都更新本文件。

## 待更新里程碑
- [x] 检查最新提交
- [x] 定位第一个未完成任务
- [x] 判断是否需要任务分解
- [x] 完成实现
- [x] 完成验证
- [x] 更新 `TODO.md` / `PLAN.md`
- [ ] 提交并停止

## 进展记录
- 已检查最新提交 `be1747658a03b41da6a49903799107e68c266699`，提交信息为 `[T1510c1] Add extern native-roots moving-GC blocker`。该提交没有修复实现，而是把既有问题显式加入 `TODO.md` / `PLAN.md`，因此需要先处理该 blocker。
- 已确认 `TODO.md` 中当前第一个未完成任务是 `T1510c1`：修复 `@Extern` + `enter_native` 在 moving GC 下把过期 SSA keepalive 写回 managed 局部槽位。
- 已检查 `PLAN.md`：当前轮次顺序与 `TODO.md` 一致，`T1510c1` 是继续推进 `T4016R` 前的前置修复项。
- 当前执行决策：不再分解任务，先复现 `tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`，然后检查 extern-call lowering / native roots / spill-reload 相关代码路径，定位 stale SSA 写回发生位置并修复。
- 已复现原始失败：`cargo run -p scoop -- build tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop -o /tmp/extern_enter_native_roots_gc.out && SCOOP_GC_MOVE=1 /tmp/extern_enter_native_roots_gc.out` 退出码为 `3`。
- 已抓取 LLVM IR 并确认当前坏模式：
  - extern/native 三连调用被 statepoint 包裹；
  - 调用前从 `%x` load 出 `gc_root_keepalive_*`；
  - native 侧 moving GC 更新 `native_roots` 后，managed 侧又把 keepalive SSA store 回 `%x`，导致 stale 指针 resurrect。
- 进一步做了最小复现验证，确认问题不只发生在“写回原局部槽位”：
  - 若外层表达式先求值出一个带 GC refs 的中间值，再在后续子表达式里进入 extern/native 并触发 GC，该中间值继续保留在 SSA 里也会 stale；
  - 已用临时样例 `Box(x, @Unsafe do { gcCollectInNative(); 1 })` 复现同类崩溃。
- 因此修复策略已扩展为两层：
  1. extern/native 调用点不再复用 ordinary safepoint 的“SSA keepalive spill/writeback”合同；
  2. 对任何需要跨后续子表达式保存、且底层包含 GC refs 的中间值，先落到临时 slot，再在最终消费点 reload。
- 已开始实现：
  - `@Extern` / `enter_native` / `leave_native` 从 LLVM GC 视角标记为 leaf；
  - top-level extern call 走独立 lowering；
  - 调用参数、class ctor 显式参数、enum ctor 字段、struct/tuple literal 元素开始接入 deferred spill/reload 机制；
  - 新增一个 runtime_gc 回归 fixture 和一个 build/LLVM IR 断言 fixture 锁定合同。
- 已完成实现收口：
  - direct extern/native 路径不再把 statepoint `gc.relocate` 的旧 SSA keepalive 写回 managed 局部槽位；
  - pointer-shaped 临时 GC 值现在会注册为“活动中的额外 root 槽位”，同时纳入 ordinary conservative root 收集与 native_roots 收集；
  - class ctor 参数属性赋值改为按需从参数局部槽位 reload，不再通过 `stored_args` 旁路复用旧 SSA。
- 已完成验证：
  - `cargo test -p scoopc --lib`
  - `cargo run -p scoop -- build tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop -o /tmp/extern_enter_native_roots_gc.out && SCOOP_GC_MOVE=1 /tmp/extern_enter_native_roots_gc.out`
  - `cargo run -p scoop -- build tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop -o /tmp/extern_enter_native_gc_arg_spill_reload.out && SCOOP_GC_MOVE=1 /tmp/extern_enter_native_gc_arg_spill_reload.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档/计划同步：
  - `TODO.md` 已将 `T1510c1` 标记为 `[DONE]`，并补记 direct GC SSA reload 与 LLVM IR regression 的验收结果。
  - `PLAN.md` 已把 P0 更新为完成状态，并将下一步顺序推进到 `T4016R`。
- 当前仅剩：检查最终 diff，提交本轮变更，然后停止。
