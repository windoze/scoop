# Claude Plan

## 约束说明

- 按要求先记录执行计划，再进行仓库检查与实现工作。
- 不写入私有推理细节；此文件记录可审计的计划、决策摘要、进度与结论。

## 初始执行计划

1. 检查最新一次提交，确认提交信息或关联改动中是否明确提到已有问题；若发现，需要先修复这些既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 评估该任务规模与依赖：
   - 如果任务可在本轮完整完成，则直接实现。
   - 如果任务过大或被前置缺陷阻塞，则拆分任务，更新 `PLAN.md` 与 `TODO.md`，并只处理拆分后的第一个子任务。
4. 实现当前目标任务，同时避免以规避、兼容层或仅测试夹具方式掩盖规范偏差。
5. 运行相关验证：
   - 最小必要测试；
   - `cargo test --all`（如影响范围要求）；
   - `cargo clippy --all-targets -- -D warnings`（按要求保证无 warning）；
   - 需要时补充定向 fixture / 集成测试。
6. 更新文档与跟踪文件：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时重排任务依赖；
   - 在 `PLAN.md` 中记录状态、拆分结果或阻塞原因；
   - 持续更新本文件的进度日志。
7. 使用清晰的提交信息提交本轮变更，然后停止，不继续下一个任务。

## 进度日志

- 已创建本文件并记录初始计划。
- 已检查最新提交 `f5a594d75b08151c1ccc80197832dd266152dc83`、`TODO.md`、`PLAN.md` 与工作区状态。
- 已确认最新提交显式插入的既有问题就是当前首个未完成任务 `T4007S`：修复 `cargo test --all` 中 `gc_immix_compaction` 两条测试的 STW / park 协调挂起。

## 当前任务：T4007S

### 目标

- 排查并修复 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 中：
  - `immix_compaction_does_not_move_pinned_objects`
  - `immix_compaction_updates_native_roots_slots_and_object_fields`
- 恢复：
  - `cargo test -p scoop_runtime immix_compaction_does_not_move_pinned_objects -- --nocapture`
  - `cargo test -p scoop_runtime immix_compaction_updates_native_roots_slots_and_object_fields -- --nocapture`
  - `cargo test --all`

### 当前判断

- 挂起日志是 `waiting for park: epoch=2 parked=0 need=1`，说明 GC 线程在等待某个 mutator 进入 park，但计数一直未满足。
- 两条失败都位于 runtime compaction 测试，优先怀疑：
  - STW 协议中的注册线程数 / parked 计数不一致；
  - 测试线程或辅助线程在 safepoint 协议外持有 mutator 状态；
  - pinned / native roots 路径在 compaction 前后遗漏一次 handshake 或状态清理。

### 执行步骤

1. 阅读 `gc_immix_compaction` 测试与 runtime 中 STW / park / epoch / pinned object 相关实现。
2. 复现实验：
   - 先单独跑两条测试，确认当前卡点、日志与复现稳定性；
   - 必要时加更细的定向日志或断言帮助定位。
3. 实现修复，优先保持 STW 协议主线统一，不做测试专用旁路。
4. 运行定向测试，再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 排查结果

- 已阅读 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 与 `runtime/c/scoop_gc_backend_immix.c` 中线程注册、`enter_native/leave_native`、STW begin / safepoint / park 协调实现。
- 当前基线下未能复现先前记录的 `waiting for park: epoch=2 parked=0 need=1` 挂起：
  - `cargo test -p scoop_runtime immix_compaction_updates_native_roots_slots_and_object_fields -- --nocapture` 通过；
  - `cargo test -p scoop_runtime immix_compaction_does_not_move_pinned_objects -- --nocapture` 通过；
  - `cargo test -p scoop_runtime --test gc_immix_compaction -- --nocapture` 通过；
  - `cargo test --all` 通过；
  - `cargo test -q -p scoop_runtime --test gc_immix_compaction` 连续复验 20 轮，均通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 结论：`T4007S` 在当前仓库基线已不再是现行缺陷，而是一个陈旧的验证 blocker；本轮无需修改 runtime 代码，只需要更新任务跟踪状态并切换到下一项 `T4007R`。

## 收尾动作

1. 将 `T4007S` 标记为完成，并在 `TODO.md` / `PLAN.md` 中写明复验结论。
2. 记录本文件的最终结论。
3. 提交文档状态更新，停止本轮。
