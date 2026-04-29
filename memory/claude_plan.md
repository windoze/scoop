# Claude Plan

## 约束说明

- 不写入详细内部思维过程；此文件只记录可执行计划、决策和进度。

## 当前计划

1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务。
5. 运行相关测试与必要的质量检查，修复发现的问题。
6. 更新 `memory/claude_plan.md`、`PLAN.md`、`TODO.md` 记录进度。
7. 按仓库风格提交一次 git commit，然后停止。

## 关键实现决策

- 最新提交 `[T5001d1R] Review explicit frame layout coverage` 未记录新的待修 pre-existing issue；当前首个未完成任务仍为 `T5001d2`。
- 不拆分 `T5001d2`：延续当前“入口先发射占位 raw frame alloca，函数收尾补全 descriptor / frame 大小，并统一注入 entry setup / return pop”的实现路线。
- 本轮先以 correctness 为准：在 safepoint/native 边界前把 conservative roots 写入 explicit frame slot，边界返回后写回 spill slot 并把 frame slot 清回 `NULL`。
- 若测试表明存在某类退出路径、临时 roots 或已有 lowering 无法映射到 frame slot，则先把它当作真实 blocker 修复，不以缩小覆盖面规避。

## 进度

- 已创建计划文件。
- 已检查最新提交、`TODO.md`、`PROMPT.md` 与 `PLAN.md`；当前目标为 `T5001d2`。
- 已确认当前未提交改动已覆盖 activation frame object/TLS 生命周期的大部分骨架，并新增了对应 LLVM 回归。
- 已复核最新提交 `[T5001d1R] Review explicit frame layout coverage`，未发现提交说明中带出的待修既有问题。
- 已确认当前实现路线为：给 tracked stack-backed root slot 预留 explicit frame mirror，在 safepoint 前写入 frame slot，离开 safepoint与函数返回时清回 `NULL`，并在函数 entry/return 自动接入 TLS push/pop。
- 已通过定向 LLVM 回归确认 managed function 会发射 TLS push/pop，并在 safepoint 前后正确写入/清理 explicit frame slot。
- 已通过 `cargo check -p scoopc`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/build` 验证当前实现。
- 下一步更新 `PLAN.md` / `TODO.md`，将 `T5001d2` 标记完成并准备提交。
