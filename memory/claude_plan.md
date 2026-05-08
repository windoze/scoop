## 当前执行计划

约束说明：按安全与隐私要求，这里记录可执行计划、关键判断与进度，不记录原始私有思维链路。

1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务；将其视为本次唯一目标。
2. 检查最近提交是否直接提到与该任务相关且未完成的问题；若是，则将其作为该任务的一部分或在 `TODO.md` 中补成前置依赖。
3. 阅读该任务涉及的条目、依赖、验证要求，以及必要的代码与测试位置，避免开放式问题排查。
4. 实现该任务要求；若遇到阻塞且无法在当前任务内直接修复，则按要求在 `TODO.md` 中插入最小前置任务并停止。
5. 运行与该任务直接相关的验证，至少包含任务要求的测试；若改动影响通用质量门禁，则补跑对应命令。
6. 更新文档记录：
   - 在 `TODO.md` 中将完成的任务标题前缀改为 `[DONE]`，补全 completion record。
   - 仅当阶段计划、依赖或完成标准变化时更新 `PLAN.md`。
   - 在本文件持续记录关键进展与计划变更。
7. 按仓库提交风格创建一次 git commit，提交本次任务的全部相关未提交改动，然后停止。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md` 并确认首个未完成任务是 `CG-T07S0a15`：修复 `stdlib_hash_set_map_basic` 中 `scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved `T`。
- 已检查最近提交：`[CG-T07S0a14] Preserve concrete smart-cast member access types`，未发现需要先并入当前任务的显式未完成事项。
- 下一步：复现 `stdlib_hash_set_map_basic` 的 build/test 失败，定位 materialized MIR 中 `__map_alloc_empty_table` 的 array transport element type 何处未具体化，并沿 authoritative materialize/transport contract 主线修复；若发现当前任务无法直接落地且存在新的真实前置缺口，则回写 `TODO.md` 后停止。
- 已复现：`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic` 失败，报 `scoop.collections.__map_alloc_empty_table` 的 `array transport element type` 仍为 unresolved `T`。
- 已完成首轮修复：更新 `crates/scoopc/src/mir/materialize.rs` 的 `repair_array_call_transport_types()`，在 `BuilderBuildArray` / `BuilderBuildMutableArray` 上优先回读 target local 或 call result transport 的 concrete array type，在 `Get` / `Set` 上优先回读 receiver operand type；随后再刷新 array element transport contract。
- 已补回归测试：新增 materialize 单测，锁定 `stdlib_hash_set_map_basic` 中 `scoop.collections.__map_alloc_empty_table` 的 `__scoop_array_builder_build_mutable_array` transport metadata 必须具体化为 `MutableArray<Int>` / `Int`。
- 第二轮修复：发现真正遗漏的是 alias receiver `MutableMap.set(...)` 的 array transport repair 仍试图从 `array_ty` 反推 element type；对于 `scoop.collections.MutableMap` 这类 typealias，`array_ty` 本身不是 canonical `MutableArray<Int>`，因此仍会保留 unresolved `T`。已改为在 `Get` 上优先回读 target/result type、在 `Set` / `BuilderPush` 上优先回读 value operand type，从而不依赖 alias 展开。
- 定向验证结果：新增单测通过，`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic` 通过。
- 新 blocker：同一 fixture 的 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 仍失败，但已不再是 materialized MIR unresolved `T`。通过临时最小复现确认：`MutableSet` 在 `remove(3)` 后再 `add(24)` 仍保持 `contains(0/8/24)` 正确；真正漂移出现在只读视图 `val ro: Set = s.asSet()` 后同一 body 组合调用 `ro.len()` / `ro.contains(...)` 时，运行期可观测值变成 `len() == 2`、`contains(0) == false`，而只打印 `ro.get(0..2)` 时又能看到导出的底层数组仍为三个元素。这说明新的独立 blocker 位于 `Set` 只读视图 / alias receiver member-call 路径，不属于本次已修复的 `__map_alloc_empty_table` unresolved-`T` 问题本身。
- 决策：按顺序约束将该运行期漂移登记为 `CG-T07S0a15` 的新前置任务，当前调用在更新 `TODO.md` 后停止；不继续把两个独立问题揉成同一任务落地。
- 已更新 `TODO.md`：插入新前置任务 `CG-T07S0a15a`，并让 `CG-T07S0a15` 明确依赖它；同时记录本次已落地的 `materialize.rs` 修复与已通过的定向验证。
- 收尾：准备检查工作区状态并按任务/阻塞提交当前改动，然后停止，等待下一次调用处理新的首个未完成任务 `CG-T07S0a15a`。
