## 执行计划

说明：不写入逐字内部思维过程；此文件记录可执行计划、关键判断依据与进度更新，便于跟踪本次任务。

1. 读取 `TODO.md`，定位首个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交是否直接提到与该任务相关的未完成问题；若是，则将其视为任务的一部分或在 `TODO.md` 中补成前置任务。
3. 阅读与当前任务直接相关的代码、测试、规范和任务说明，确认约束、依赖与验收要求。
4. 实现当前任务；若遇到阻塞且必须新增前置任务，则只做最小必要的 `TODO.md` / `PLAN.md` 调整并停止。
5. 运行与当前任务相关的验证；若任务要求或改动范围需要，则运行更完整的测试与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`：仅在任务真正完成时给任务标题加上 `[DONE]`，并补全完成记录；若只是阻塞，则保持未完成并记录前置任务。
7. 仅当阶段计划或依赖关系发生变化时更新 `PLAN.md`。
8. 检查工作区状态，按要求创建一次提交，提交信息使用当前任务号。
9. 停止，不继续下一个任务。

## 进度

- 已写入初始执行计划，下一步读取 `TODO.md` 确认当前任务。
- 已确认首个未完成任务是 `CG-T07S0a3`：修复 `gc_trace_task_field_basic.scoop` 中 `Async.await(holder.task)` perform site metadata 的 payload transport type / payload component type 漂移。
- 已检查最近一次提交：`[CG-T07S0a2] Fix String surface drift and record task blocker`。该提交已把 `CG-T07S0a3` 作为顺序上的新 blocker 补录到 `TODO.md`，与当前任务直接相关；当前按 `CG-T07S0a3` 继续执行即可，无需再新增前置任务。
- 下一步：复现 `gc_trace_task_field_basic.scoop` 的 build 失败，并定位 direct-style MIR perform site metadata 的发布链路。
- 已复现失败，报错固定为 `main` 的 `Async.await(holder.task)` perform metadata：`payload transport type disagrees with payload component type`。
- 已定位根因：typed HIR 中 `holder.task` 的 `MemberAccess` 结果类型被 HIR lowering 默认发布成 `Any`，导致 `PerformSiteContract.payload_component_tys = [Any]`；而 MIR member-value lowering 仍能得到真实的 `Task<String>`，于是 direct-style MIR validator 在 perform metadata 上报漂移。
- 已开始修复：让普通 member access 与 `?.` 内部 member access 使用 authoritative result type，而不是默认 `Any`；同时补 direct-style MIR 回归测试覆盖 `gc_trace_task_field_basic.scoop` 的 `Async.await(holder.task)` payload type。
- 定向验证已通过：`cargo test -p scoopc refactor_mir_task_field_perform_contract_keeps_task_payload_type`、`cargo run -p scoop -- build tests/fixtures/run-pass/gc_trace_task_field_basic.scoop -o /tmp/gc_trace_task_field_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`。
- full-suite 已越过 `gc_trace_task_field_basic.scoop`。本次改动同步带出 HIR/MIR snapshots 漂移，已更新 `delegated_property_lowering.hir`、`member_access.hir`、`safe_call_not_null_assert.hir`、`aggregate_transport.mir`。
- 默认 `cargo run -p scoop -- test` 现在停在新的独立 blocker：`tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`，build 阶段 panic `assignment place contract references an unallocated local: S34`。已按顺序在 `TODO.md` 新增 prerequisite `CG-T07S0a4`，并将 `CG-T07S0a3` 标记为 `[DONE]`。
- `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings` 已通过。下一步：检查工作区状态并创建 `CG-T07S0a3` 提交，然后停止。
