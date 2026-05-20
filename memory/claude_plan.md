# 执行计划

## 当前约束

- 以 `TODO.md` 为任务顺序和完成状态的唯一来源。
- 只完成第一个未标记 `[DONE]` 的任务，完成后提交并停止。
- 若发现当前任务存在必须先修复的阻塞问题，则在 `TODO.md` 中加入最小必要前置任务，提交后停止。
- 不使用规避方案；相关规范不匹配必须修复或记录为前置任务。

## 步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题。
3. 阅读该任务相关代码、测试和文档，确认验收要求。
4. 实现当前任务或必要的阻塞修复。
5. 运行与任务相关的测试；如有失败，定位并修复。
6. 更新 `TODO.md`：给完成任务标题加 `[DONE]`，并补充完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 运行必要的最终验证。
8. 检查 `git status`、`git diff`、最近提交，提交本次任务相关改动。
9. 停止，不继续下一个任务。

## 进度记录

- 已创建初始执行计划；下一步读取 `TODO.md` 定位首个未完成任务。
- 已读取 `TODO.md`，首个未完成任务为 `P8-T02：迁移 scoop.runtime.test native helpers`。
- 下一步检查最近提交是否有直接相关未完成事项，并阅读 P8 计划与现有 `scoop_test_*` 实现。
- 最近提交 `f441c04b [P8-T01] Establish public runtime header` 未在提交信息中声明未完成事项。
- 已确认普通程序当前仍链接多类 `scoop_test_*`：`runtime/c/scoop_test.c`、GC backend 内部 smoke、sync destroy 计数、thread spawn gate。
- 实施重点调整为：先把独立 `scoop_test.c` helper 挂到 `scoop.runtime.test` cone 的 native-build，再处理 runtime core 中仍导出的 test-only 符号，最后用普通程序 `nm` 检查无 `scoop_test_*`。
- 已完成迁移：`scoop.runtime.test` 现在通过 cone native-build 编译 `native/scoop_test.c`；普通 runtime 编译排除 GC test helpers，sync test counters 由 test cone override hook 提供，thread spawn gate test exports 删除。
- 已完成验证：runtime GC fixtures、run-pass suite、full fixture suite、runtime allowlist、普通链接符号检查、`cargo build`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 均通过。
- 下一步更新 `TODO.md` 标记 `P8-T02` 完成并写入完成记录，然后提交。
- 已更新 `TODO.md`：`P8-T02` 标记为 `[DONE]`，顶部状态更新为下一任务 `P8-T03`，并补充完整完成记录。
- 下一步检查 git status/diff/log，确认只提交本任务相关改动。
