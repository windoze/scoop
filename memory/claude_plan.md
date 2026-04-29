## 当前执行计划

### 目标
本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

### 初始思路
1. 先检查最新一次提交，确认提交信息里是否提到需要先处理的既有问题；如果提到，就先修复该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划与任务依赖关系。
4. 判断该任务是否过大；如果过大，则把它拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
5. 实现任务时，若在探查、测试、审查过程中发现任何既有缺陷、规格不匹配、实现边界缺失或回避式实现，立即优先处理；若无法在本轮直接修复，则把它作为前置任务插入 `TODO.md` 的正确位置，更新 `PLAN.md`，提交后停止。
6. 对本轮任务做充分验证，优先运行与改动直接相关的测试；若任务影响范围较大，再补充更广泛的检查，包括 `cargo clippy --all-targets -- -D warnings`（如果改动范围要求这样做）。
7. 完成后更新 `TODO.md` 与 `PLAN.md`，记录完成情况和任何计划调整。
8. 按仓库约定创建一次 git 提交，然后停止，不继续做下一个任务。

### 执行注意事项
- 不通过缩小范围、替换表示方式、规避坏路径、夹带临时特判等方式绕过问题。
- 不回退或覆盖我未创建的现有改动。
- 在关键步骤完成或计划变化后，及时更新本文件。

### 待执行的首批检查
1. 查看最新提交信息与相关变更线索。
2. 查看 `TODO.md`。
3. 查看 `PLAN.md`。
4. 查看当前工作区状态，避免误动无关文件。

## 进展更新（已完成检查）

- 最新提交为 `[T5001aR] Review roots pipeline baseline`，提交信息未声明必须先修的既有问题。
- `TODO.md` 中首个未完成任务为 `T5001b 抽出统一 runtime root map / slot visitor 抽象`。
- `PLAN.md` 与 `TODO.md` 一致：本轮先做 runtime 侧统一 root map 抽象，不切换默认模式。
- 当前工作区仅有 `memory/claude_plan.md` 修改，属于本轮计划记录。

## 针对 T5001b 的细化执行方案

1. 先把 runtime 内“managed frame roots 来源”的 stackmap 专用入口抽成一个内部 root-map 抽象，保持 header-only，避免新增 ABI 导出符号。
2. 在该抽象里保留并列实现边界：
   - 当前可用实现：stackmap root map；
   - 预留实现：explicit-frame root map（先预留 kind/接口，不在本轮落地行为切换）。
3. 将 `scoop_gc_backend_immix.c` 中 verify-roots、major/minor mark、roots update 等上层路径改为调用统一 root-map visitor，而不再直接调用 stackmap 专用函数。
4. 将 `scoop_gc.c` 中对应 baseline 路径同步改为使用同一抽象，避免 backend 之间继续分叉。
5. 若需要更新构建追踪（例如新增内部头文件），同步更新 `crates/scoop_runtime/build.rs` 的 `rerun-if-changed`。
6. 运行与 runtime/GC 相关的最小充分测试与静态检查；若发现既有缺陷，先修复再继续。
7. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，并提交一次 git commit。

## 进展更新（T5001b 已实现并验证）

- 已新增 `runtime/c/scoop_gc_root_map_internal.h`：
  - 定义 `ScoopGcManagedRootMap`；
  - 定义统一 visitor 入口 `scoop_gc_root_map_visit_slots(...)`；
  - 当前接入 stackmap root map；
  - 预留 `SCOOP_GC_MANAGED_ROOT_MAP_EXPLICIT_FRAME` 作为后续并列实现边界。
- 已将 `runtime/c/scoop_gc_backend_immix.c` 的 managed roots 上层消费点切到统一 root-map 抽象，覆盖：
  - verify-roots；
  - major GC mark；
  - minor GC mark；
  - major/minor roots update；
  - stackmap roots smoke 测试路径。
- 已将 `runtime/c/scoop_gc.c`（baseline backend）对应路径同步切到同一抽象，避免 backend 分叉。
- 已更新 `crates/scoop_runtime/build.rs`，确保新内部头文件变更会触发重编译。
- 期间发现 `immix` backend 新引入了多处 `records_hit` 未使用告警，已在本轮一并清理，以满足“无 warnings”要求。

## 已执行验证

1. `cargo test -p scoop_runtime`
2. `cargo clippy --all-targets -- -D warnings`

结果：通过；未再出现本轮改动引入的编译告警。

## 收尾步骤

1. 将 `T5001b` 标记为完成，并在 `PLAN.md` 记录完成状态。
2. 检查工作区 diff 与 git 状态，确认仅包含本轮任务相关变更。
3. 按仓库提交规范创建一次提交，然后停止。
