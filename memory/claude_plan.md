# Claude Execution Plan

## Scope

本次调用只完成 `TODO.md` 中按顺序出现的第一个未完成任务。完成后更新任务记录、验证、提交并停止，不继续处理下一个任务。

## Reasoning Summary

- `TODO.md` 是任务顺序、完成状态、依赖和验证要求的唯一权威来源。
- 任务标题只有显式带 `[DONE]` 才算完成； completion record 中的文字不能替代标题状态。
- 在确认当前任务前不做开放式历史问题扫查。
- 如果当前任务被具体缺陷、缺失语言特性或未排期失败测试阻塞，先修复该问题，或把最小必要前置任务插入 `TODO.md` 后提交并停止。
- 不用变窄范围、替代表示、夹具专用 hack 或其他 workaround 来绕过规范不匹配。

## Step-by-Step Plan

1. 读取 `TODO.md`，按文件顺序找出第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如相关，将其纳入当前任务或作为前置项写入 `TODO.md`。
3. 阅读当前任务涉及的代码、测试、夹具和文档，只收集完成该任务所需上下文。
4. 如任务可直接完成，进行最小正确实现；如遇具体阻塞，按要求更新 `TODO.md` 并停止在阻塞处理提交之后。
5. 添加或更新最小相关测试/夹具，避免通过 workaround 降低规格。
6. 运行 `cargo fmt`。
7. 运行 `cargo clippy --all-targets -- -D warnings`。
8. 在 lint 通过后运行当前任务要求的相关测试；必要时运行完整 `cargo test --all --all-targets` 和 `cargo run -p scoop -- test`，完整套件超时至少 30 分钟。
9. 若观察到未排期失败测试/夹具，修复它或在 `TODO.md` 中排入最小必要任务，且不把当前任务标记为完成。
10. 成功完成后，在 `TODO.md` 当前任务标题前加 `[DONE]`，更新 completion record；仅在阶段计划真实变化时更新 `PLAN.md`。
11. 查看 `git status`、`git diff` 和最近提交，确认变更范围。
12. 用符合仓库风格的提交信息提交所有本次任务相关变更。
13. 停止，不处理后续任务。

## Progress Log

- 初始计划已写入，下一步读取 `TODO.md` 确认当前任务。
- 已确认本轮任务为 `P10-T04-c-3`：consumer pipeline 在 cache-hit 时剔除 dep AST 的中端适配。`P10-T04-c` 是已拆分父项，本轮不收口父项或 `P10-T04-c-4`。
- 下一步读取 `frontend.rs`、cached cone import/effect facts builder、相关测试与 artifact handoff 代码，确认 cache-hit 后仍把 dep AST 放入 `active_sources`/`build_closure_sources` 的具体路径。
- 已修改 `frontend.rs`：dependency cone artifact cache hit 成功读取后立即发布 cached artifact 并 `continue`，不再执行该 dep cone 的 AST load / resolve / typecheck / artifact rewrite；旧反向测试已改为正向测试，使用语法损坏的 dep source 验证 cache-hit 不读取源，并断言 dep source 不在 output build closure 中。
- 验证进展：`cargo fmt`、定向 frontend cache-hit 测试、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 均通过；常规 `cargo run -p scoop -- test` 在当时的子进程二进制状态下通过 1536 checks。
- 显式重建 `target/debug/scoopc` 后重跑 `source_path_dependency_public_call` cold/warm/edit 手工场景，cold build 已进入 c-3 目标路径（dep AST 不再由 consumer 重新 lower），并在链接阶段因 dep `.o` 未加入 link 命令出现 unresolved dep callable symbol；该失败精确落在已排期的 `P10-T04-c-4`（consumer link 阶段拉 dep `.o` 与跳过 dep body emit），本轮不做 workaround 或提前实现下一任务。
- 已更新 `TODO.md` 与 `TODO-7.md`：`P10-T04-c-3` 标记 `[DONE]`，完成记录写明实现、测试与 fresh 子进程下的 `P10-T04-c-4` 链接边界；下一任务为 `P10-T04-c-4`。
