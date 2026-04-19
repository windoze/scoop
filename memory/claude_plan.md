# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中“第一个未完成任务”，并在完成后停止。

## 约束与执行原则

1. 在开始任何实现前，先检查最新提交是否提到已有问题；若有，先修复这些问题，再进入任务执行。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大或依赖不满足，则先进行任务拆分或依赖调整，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 严格避免以临时绕过、测试专用补丁、偏离规范的方式“完成”任务。
5. 若发现规范不匹配、实现缺口或前置依赖缺失，必须先把该问题整理为新的待办任务，更新排序与依赖，再提交并停止。
6. 本轮结束前需要：
   - 完成实现
   - 运行相关测试与必要的质量检查
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 视进展更新本文件
   - 提交 Git commit

## 具体步骤

1. 查看最新一次 Git commit，确认是否包含“已知问题 / follow-up / FIXME / broken / TODO”等需要先处理的信息。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定当前优先级最高的未完成任务，以及是否已有拆分或依赖说明。
3. 结合代码现状评估该任务：
   - 若可以直接完成，则进入实现；
   - 若过大，则拆分成更小子任务，并把第一个子任务作为本轮目标；
   - 若被规范缺口阻塞，则先把阻塞项写入 `TODO.md` 并调整顺序。
4. 阅读与目标任务相关的代码、测试、规格说明和最近改动，确认实现边界。
5. 实施代码修改，必要时补充/调整测试。
6. 运行最小充分的验证：
   - 目标相关测试
   - 必要时运行更大范围测试
   - 必要时运行 `cargo fmt`
   - 按要求运行 `cargo clippy --all-targets -- -D warnings`
7. 根据结果继续修复，直到当前目标任务满足要求，或明确确认存在前置阻塞并已转化为待办。
8. 更新文档与计划文件：
   - 在 `TODO.md` 标记已完成，或在阻塞场景下重排任务
   - 在 `PLAN.md` 记录当前状态与后续安排
   - 在本文件记录关键进展与计划变更
9. 检查工作区变更，确保仅包含本轮合理修改，然后创建一次清晰的 Git 提交。
10. 停止，不继续处理下一个任务。

## 当前状态

- 已创建本轮计划文件。
- 已查看最新提交：`52bc544446c9db62fbfae4c8919ad8f2be54b2d3`，提交说明未显式留下需要先于 `TODO` 执行的新遗留修复。
- 已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`。
- 已确认本轮首个未完成任务是 `T4008R`：`Review：确认 effect 完整性收口没有引入新的 shape-based lowering`。

## 针对 T4008R 的执行细化

1. 阅读 `T4008a` 到 `T4008c4` 涉及的 typecheck / HIR / LLVM / runtime 主线，重点核查：
   - `Continuation.resume` 是否仍经由普通 call binder / payload-shape 规则，而非独立 surface 特判。
   - handler arm head 的 effect/op type args、binder 和 perform payload 是否仍复用统一绑定/transport 主线。
   - effect codegen 是否存在按源码形状、参数个数或特定语法节点分流的新增补丁。
   - `ImmediateResume` 是否已被明确记录为 deferred optimization，而不是继续与规范形成静默分裂。
2. 运行最小充分验证：
   - `cargo run -q -p scoop_tools -- spec-fixtures check`
   - `cargo run -q -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
3. 若 review 发现实现裂缝：
   - 先定位是否属于既有 bug / 缺失前置能力；
   - 按要求更新 `TODO.md` / `PLAN.md` / 本文件；
   - 提交后停止，不继续进入 `T4009`。
4. 若 review 未发现新的阻塞裂缝：
   - 将 `T4008R` 标记为完成；
   - 在 `PLAN.md` 记录 review 结论与验证结果；
   - 提交后停止。

## 本轮执行结果

- 已完成代码审查，未发现需要在 `T4009` 之前插入的新 blocker。
- review 结论：
  - handler arm head、receiver effect op 与普通 effect-op call 继续复用共享签名 lowering / `arg_mapping` / payload tuple metadata 主线。
  - `Continuation.resume` 继续依赖 typecheck 写回的 call-site side table 与共享 call binder helper，不存在新的按 member 名、receiver 形态或 payload 个数补丁选路。
  - `ImmediateResume` 当前仍统一走 GC-managed full-machine lowering，stack-local fast path 只作为 deferred optimization 记录；spec/runtime/sysroot 表述一致。
- 已完成验证：
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo run -q -p scoop -- test` → `fixtures: ok (1070)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已更新：
  - `TODO.md`
  - `PLAN.md`
  - 本文件
- 下一步仅剩：检查 diff、创建 Git commit，然后停止。
