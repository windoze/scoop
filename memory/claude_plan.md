# 本轮执行计划（决策摘要）

说明：这里记录可执行计划、关键判断和进度更新，不写入不可审计的内部推理细节。

## 初始计划

1. 检查最新一次 Git 提交的提交信息与变更范围，确认是否提到已知问题或遗留修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和已有计划。
4. 如果首个未完成任务过大，先将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
5. 实现当前目标任务，并补充或调整必要测试。
6. 运行与本次修改相关的验证命令，至少覆盖：
   - 相关定向测试；
   - 必要时执行 `cargo test --all`；
   - `cargo clippy --all-targets -- -D warnings`（如果本次改动范围允许且时间成本合理）。
7. 更新进度文档：
   - 在 `TODO.md` 标记当前任务完成，或在受阻时按依赖顺序重排；
   - 在 `PLAN.md` 记录状态变化与后续影响；
   - 在本文件补充执行记录。
8. 使用清晰的提交信息提交本轮变更，然后停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件，等待检查最新提交、`TODO.md` 与 `PLAN.md`。
- 已确认最新提交 `[T4007R] 复审 RTTI 参数化覆盖主线` 未声明新的待修复遗留缺陷；当前工作区只有本文件的未提交修改。
- 已定位首个未完成任务为 `T4008`。该任务过大，现已决定拆分为 `T4008a -> T4008b -> T4008c -> T4008R`，并同步写回 `TODO.md` / `PLAN.md`。
- 关键验证结论：临时 probe `/tmp/t4008a_single_handle_and_then_probe.scoop` 已成功 build/run，stdout 为 `17`。这说明单个 setup `handle` 内连续两次 `Async.await` 已是可执行主线，`stdlib/task.scoop` 中 `Task.andThen` 的双 `handle` 已属于过时 workaround。
- 当前执行目标：完成 `T4008a`，即清理 `Task.andThen` 的双 `handle` workaround，并用真实 stdlib 路径重新验证相关 run-pass / 质量门禁。
- 已完成 `T4008a` 实现：`stdlib/task.scoop` 的 `Task<Int>.andThen` 已收口为单个 setup `handle` 内顺序执行两次 `Async.await`，删除了过时的双 `handle` workaround 与对应注释。
- 已完成验证：
  - `cargo run -q -p scoop -- run /tmp/t4008a_single_handle_and_then_probe.scoop` -> stdout `17`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/std_task_async_adapters_basic.scoop` -> stdout 与 golden 一致
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass` -> `fixtures: ok (349)`
  - `cargo test --all` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
- 待执行的收尾步骤：检查 diff、提交本轮变更（建议消息：`[T4008a] 移除 Task.andThen 双 handle workaround`），然后停止。
