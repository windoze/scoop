# 本轮执行计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的依赖、验证要求和完成记录，必要时查看最新提交是否包含与该任务直接相关的未完成事项。
3. 仅围绕当前任务收集代码上下文，不做开放式历史问题排查。
4. 按任务要求实现最小且完整的变更；如果发现阻塞当前任务的真实缺口，则在 `TODO.md` 中插入最小前置任务并停止。
5. 运行相关测试和质量检查；若失败且属于当前任务范围，继续修复并复测。
6. 更新 `TODO.md`：完成时在任务标题前加 `[DONE]`，并补充完成记录；只有阶段计划变化时才更新 `PLAN.md`。
7. 提交本轮所有相关变更，提交信息使用任务编号和简明说明。
8. 完成一个任务后停止，不继续下一个任务。

## 当前任务

- 已读取 `TODO.md` 与 `TODO-5.md`。
- 本轮第一个未完成任务为 `P13-T04`：最终 fixture 收尾，要求仓库内所有仍存在的 fixture 全量通过。
- 执行边界：本任务只做 fixture 收尾；如果发现实现层/spec 层不一致，不绕过、不弱化 fixture，按要求在 `TODO.md` 增加最小前置任务并停止。
- 不主动改 `PLAN.md`，除非发现阶段级依赖或完成标准需要变化。

## 执行计划

1. 检查最新提交与工作树状态，确认是否有直接相关未完成事项以及需要一并提交的遗留变更。
2. 运行完整 fixture suite：`cargo run -p scoop -- test`，超时设置不低于 30 分钟。
3. 将全量失败 fixture 路径写入 `target/reshape-baseline/p13t04-failing.txt`；若无失败，记录为空清单或删除/保留空文件以满足验证说明。
4. 对照 `TODO-1.md` 至 `TODO-5.md` 中“待 P9-T02 / P13-T04 处理”清单，确认是否有历史待处理项未闭合。
5. 若存在 failing fixture，逐条判断被测功能是否仍存在，并按任务规则改写或删除；每处理一条立即运行 `cargo run -p scoop -- test --fixtures <path>` 复验。
6. 复跑完整 fixture suite，确保 0 failing。
7. 运行 `cargo build` 与 `cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md` 索引与 `TODO-5.md` 中 `P13-T04` 完成记录，写明 failing 全集、分类/处理决定、验证结果和上游待处理清单闭合情况。
9. 运行必要的 diff/whitespace 检查，提交本轮相关变更，提交信息使用 `P13-T04` 编号，然后停止。

## 进度记录

- 计划文件已更新。
- 最新提交为 `81677b05 [P13-T03] Clean sysroot historical TODO comments`，未声明与 `P13-T04` 直接相关的未完成阻塞项。
- 当前工作树已有本轮修改 `memory/claude_plan.md`；未跟踪的 `CLOSURE_FIX.md`、`OVERLOAD_RESOLUTION.md`、`UnsupportedMainBody_FIX.md` 为既有非本任务文件，保持不修改。
- 初次 `cargo run -p scoop -- test` 结果为 `1335/1340` targets passed，5 个 failing targets，均为 HIR golden mismatch：`const_fun_basic.scoop`、`do_block_multiple_trailing_lambda_boundary.scoop`、`handle_mixed_arm_kinds.scoop`、`lowered_call_args.scoop`、`lowered_comptime_control_flow.scoop`。
- 失败清单已写入 `target/reshape-baseline/p13t04-failing.txt`。
- 已确认 5 个失败都是 P13-T03 注释清理造成的 sysroot `target_decl_span` 稳定快照漂移；被测 HIR 行为仍存在，处理方式为改写 golden，不删除 fixture。
- 已同步 5 个 `.hir` golden，并逐条运行对应 `cargo run -p scoop -- test --fixtures <path>`，全部通过。
- 修复后复跑 `cargo run -p scoop -- test` 通过：`fixtures: ok (1377)`，即 `1340/1340` targets 通过。
- 已清空 `target/reshape-baseline/p13t04-failing.txt`，`wc -l` 为 `0`。
- `cargo build` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md` 索引与 `TODO-5.md`，将 `P13-T04` 标记为 `[DONE]` 并写入完成记录。
- 提交前检查发现 `AGENTS.md` 有非本轮修改；该文件与 `P13-T04` 无关，保持不改、不纳入本次提交。
