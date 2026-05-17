# 当前执行计划

## 范围

- 本次只处理 `TODO.md` 中第一个标题未带 `[DONE]` 的任务。
- 不跳过 review 任务，不因为任务较大而默认拆分。
- 如果发现当前任务被具体前置缺陷阻塞，只添加最小必要前置任务并停止。

## 步骤

1. 阅读 `TODO.md`，确定第一个未完成任务及其验收要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 阅读与该任务相关的代码、测试和文档，确认实现边界。
4. 实施最小正确修改，避免绕过规格或弱化测试。
5. 运行当前任务要求的验证命令和必要的相关测试；如果失败，修复后重跑。
6. 更新 `TODO.md`：把已完成任务标题加上 `[DONE]`，并更新完成记录。
7. 仅在阶段计划实际变化时更新 `PLAN.md`。
8. 提交所有与本任务相关的变更，提交信息包含任务编号。
9. 停止，不继续处理下一个任务。

## 进度记录

- 已创建初始执行计划。
- 已读取 `TODO.md`，第一个未完成任务是 `P12-T01`。
- 已读取 `TODO-5.md` 中 `P12-T01` 详情：本任务要求审计 sysroot 全部 `.scoop` 文件中的顶层 fun 与 type body method，确认每个声明满足 body / `@Intrinsic` / `@Extern` 三选一，并把 file × method 矩阵写入完成记录。
- 已检查最新提交 `c1bb7713 [P11-T02] Move runtime test helpers out of core`，未发现直接要求先处理的未完成问题。
- 已枚举并结构扫描 `sysroot/**/*.scoop`。
- 初次审计发现 `sysroot/core.scoop` 中 `print<T>` / `println<T>` 是无 body、无 `@Intrinsic`、无 `@Extern` 的重复光声明；`sysroot/print.scoop` 已有普通 Scoop body。
- 已删除 core 中这两条重复光声明，审计脚本复跑为 0 条违规。
- `cargo build` 通过。
- 全量 fixture 首次复跑出现 12 个 `print/println` 不可调用失败，定位为 typecheck-only 路径只使用 `Sysroot::index_files()`，而 `print.scoop` 这类可编译 sysroot 文件未进入索引；下一步修复 sysroot 索引包含非重复的 compilable files，再重跑验证。
- 已将 `Sysroot::files` 收口为所有 sysroot 声明索引的 signature-only AST，并保留完整 AST/support source 路径；同时修复 fixture runner 与 dump/materialization 装配，避免 `print.scoop` 声明缺失或完整 body 被当作未 typecheck 的索引 AST 降低。
- 已更新 5 个 HIR golden 中因删除 core 重复签名导致的 `target_decl_span` drift。
- 最终结构审计结果：`VIOLATIONS 0`，9 条无 body 的声明均为 interface/effect 抽象 method。
- 最终验证：`cargo build`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/println_string_ok.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/hir`、`cargo run -p scoop -- test`、`cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已回写 `TODO.md` 与 `TODO-5.md`，将 `P12-T01` 标记为 `[DONE]` 并写入完成记录。
- 下一步提交本任务所有变更后停止。
