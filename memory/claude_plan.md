## 当前执行计划

说明：我不能写入隐藏的完整思维链，但会在这里持续维护可审阅的执行计划、关键发现、变更决策与进度。

1. 查看最新提交，确认是否提到了需要先修复的既有问题；若有，优先处理。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，先细化到 `PLAN.md` 与 `TODO.md`，并执行拆分后的第一个子任务。
4. 实现当前目标任务，期间若发现既有缺陷、规格不匹配或实现边界问题，立即转为优先修复或前置任务。
5. 运行相关测试与必要的质量检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态与调整。
7. 按仓库约定创建一次 git 提交，然后停止。

## 进度记录

- 已初始化执行计划文件，待检查最新提交与任务列表。
- 已检查最新提交：上一提交 `[T5001e1R]` 修复的是 production MIR safepoint reload 缺口，本次开始前没有新的明确遗留 issue 需要先插队处理。
- 已确认当前首个未完成任务是 `T5001e2`：补齐 aggregate refresh / rebuild contract，覆盖 args、returns、payload transport。
- 代码探查结论：`T5001e1` 仅收紧了“单槽 pointer-shaped GC 值”的 post-safepoint reload；`Tuple/Struct/Enum` 等含 ref 的 aggregate 仍会在多个入口直接从旧 local/spill/sret slot 做整体 `load`，存在 stale aggregate 风险。

## 当前实现方案

1. 在 LLVM codegen 中新增通用 helper：对带 explicit-frame mirror 的 storage slot，按“GC leaf 从 frame home slot reload，非 GC leaf 继续从原 storage slot 读取”重建 fresh aggregate。
2. 让所有“按值读取/传递 aggregate”的入口统一走该 helper，优先覆盖：
   - local / MIR local 读取；
   - deferred call arg materialize；
   - indirect aggregate call arg pointer materialize；
   - hidden sret result 读取；
   - effect payload boxing 等复用 deferred materialize 的路径。
3. 为上述 contract 增加定向 LLVM 回归，至少锁定：
   - safepoint 后读取 aggregate local 时不再直接 load 旧 local；
   - hidden sret aggregate 结果在 safepoint 后会从 explicit-frame leaf slot 重建；
   - aggregate call arg / payload transport 走 fresh rebuild 而非旧 spill 整体复制。
4. 运行相关测试与质量检查；若出现既有缺口，先修复再继续。
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交一次 git commit，然后停止。

## 完成情况

- 已实现 aggregate rebuild helper，并把 `local_ptr_for_use(...)`、deferred call-arg materialize、indirect aggregate call-arg pointer materialize、hidden-sret result load 统一切到 fresh aggregate contract。
- 已补三条 LLVM 回归：
  1. aggregate call arg 在 safepoint 后会从 explicit-frame leaf slot 重建；
  2. hidden-sret aggregate result 会从 explicit-frame leaf slot 重建；
  3. boxed effect payload 会从 explicit-frame leaf slot 重建。
- 已完成验证：
  - `cargo fmt`
  - `cargo test -p scoopc --lib`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  - `cargo test --all`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
- 已更新 `TODO.md` 与 `PLAN.md`，将 `T5001e2` 标记为完成；下一条首个未完成任务变为 `T5001e2R`。
