# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 本轮只处理第一个标题未带 `[DONE]` 的任务，完成后提交并停止。
- 若遇到阻塞当前任务的缺失特性、规格不匹配或实现缺口，不做绕行；将最小必要前置任务写入 `TODO.md`，提交后停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并记录其验证要求、依赖和完成记录要求。
2. 查看最近提交信息，仅判断是否有与当前任务直接相关的未完成事项。
3. 检查当前工作区状态，避免覆盖他人或用户已有改动。
4. 根据当前任务读取相关源码、测试和文档，确定需要修改的最小实现范围。
5. 实施任务；如发现当前任务被真实前置问题阻塞，则更新 `TODO.md` 中的任务依赖和顺序并停止。
6. 运行任务指定或最相关的验证命令；若失败，修复后重跑。
7. 更新 `TODO.md`：给已完成任务标题加 `[DONE]`，补充完成记录和验证结果。仅当阶段级计划变化时更新 `PLAN.md`。
8. 提交本轮全部相关改动，提交信息包含任务编号和简明说明。
9. 停止，不继续下一个任务。

## 进度

- 已读取 `TODO.md`，第一个标题未带 `[DONE]` 的任务是 `P5-T02：支持本地 source path dependency fixtures`。
- 任务要求：定义最小 path dependency 语法或 fixture 约定；支持 `bin` cone 依赖本地 `lib` cone；consumer 可解析/typecheck/codegen dependency public API；dependency internal/private 不可见；新增 positive 与 internal visibility negative fixture。
- 最近提交为 `P5-T01`，未显式记录与本任务相关的未完成 blocker；工作区除本计划文件外无其它未提交改动。
- 已选择 manifest 语法：`[dependencies]` 中使用 `"cone.name" = { path = "relative/path" }` 表达本地 source dependency，路径相对声明方 cone root。
- 已开始实现：manifest parser 新增 path dependency spec；source cone graph 从 consumer/local lib manifest 递归加载本地 `lib` dependency，并保留 dependency edges；新增 run-pass cone fixtures 覆盖 public 调用、internal 不可见、private 不可见。
- 验证中发现并修复了一个直接相关问题：默认 cone 增量构建 fingerprint 未覆盖本地 path dependency sources，可能导致 dependency 修改后误判 cache hit；已将 build fingerprint schema 升到 v2 并纳入 local dependency manifests/sources。
- 已完成验证：`cargo fmt`、manifest/graph/incremental 定向单测、三个新增 fixtures、`run_pass_cone` 全套、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、完整 `cargo run -p scoop -- test` 均通过。
- 已更新 `TODO.md`，将 `P5-T02` 标记为 `[DONE]` 并补充完成记录；`PLAN.md` 未变化。
- 下一步检查 diff/status，然后提交本轮改动并停止。
