# 当前执行计划

说明：本文件记录可审阅的执行计划和进度更新，不记录隐藏推理过程。

## 初始计划

1. 读取 `TODO.md`，按文档顺序识别第一个标题未带 `[DONE]` 的任务。
2. 检查最新提交信息；仅当其明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或在 `TODO.md` 中登记为前置任务。
3. 阅读当前任务相关的源码、测试、夹具和文档，确认任务要求、依赖和验证命令。
4. 如发现阻塞当前任务的缺失语言特性、规格偏差或未排期失败测试，在 `TODO.md` 中加入最小必要前置任务并停止；否则完整实现当前任务。
5. 按要求运行验证：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`，再运行相关测试；需要时运行完整测试和夹具套件。
6. 更新 `TODO.md`：将完成任务标题加 `[DONE]`，并填写完成记录。仅当阶段级计划变化时更新 `PLAN.md`。
7. 检查 `git status` 和差异，提交本次任务涉及的全部变更，提交信息包含任务编号。
8. 完成一个任务后停止，不继续处理下一个任务。

## 进度

- 已创建初始计划，下一步读取 `TODO.md` 确认首个未完成任务。
- 已读取 `TODO.md`，首个未完成任务确认为 `P10-T04-c-4`：consumer link 阶段拉取 dep `.o` 并跳过 dep body emit。
- 下一步读取 `TODO-7.md` 中该任务正文，并检查近期提交是否包含直接相关的未完成事项。
- 已读取 `TODO-7.md` 的 `P10-T04-c-4` 正文；最新提交为 `Update plan`，未明确提出与该任务直接相关的未完成实现项。
- 当前执行重点：让 consumer LLVM body emit 只发射自身 callable，并在 link 阶段把 cache-hit/cold dep artifact 的 `manifest.object_files` 加入 native link 输入。
- 已实现第一轮代码改动：`run_codegen_and_link` 追加 cached dep object 文件；LLVM body emission 跳过 cached dep callable carrier shell 定义，保留外部声明。
- 下一步按顺序运行 `cargo fmt`、针对性测试，再进入 clippy/full test/fixture 验证。
- 验证中发现目标 fixture 首次失败已从 unresolved symbol 变为重复 `_scoop_thread_init_current`；修复方式为让 single-cone LibMode object 不定义该进程级线程初始化入口，consumer executable object 继续定义。
- 最终验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、目标 cone fixture、完整 `cargo run -p scoop -- test`，以及 cold→warm→consumer edit 手工 reproducer。
- 已更新 `TODO.md` / `TODO-7.md`，将 `P10-T04-c-4` 与收口主任务 `P10-T04-c` 标记为 `[DONE]` 并记录完成内容。
