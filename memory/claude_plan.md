# Claude Execution Plan

我不能记录私密逐步推理链；本文件记录可审计的执行计划、关键决策和进度。

## 当前目标

按照 `TODO.md` 的权威顺序，只完成第一个标题未带 `[DONE]` 的任务，完成后更新记录、验证并提交，然后停止。

当前首个未完成任务：`P3-T02`，实现 `lib/syslib` 无 entry point 加载规则。

## 执行步骤

1. 读取 `TODO.md`，按文件顺序确认第一个未完成任务及其依赖、验证要求和完成记录格式。
2. 查看最近提交信息，仅在它明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 针对当前任务检查相关代码、测试和文档，避免做开放式历史问题扫描。
4. 如任务可直接完成，做最小且完整的实现；如发现阻塞当前任务的缺失特性或规格不匹配，则在 `TODO.md` 插入最小必要前置任务并停止。
5. 为实现添加或更新最小相关测试/fixtures，并运行任务要求的验证命令；遇到失败则修复后重跑相关验证。
6. 任务完成后，在 `TODO.md` 的任务标题前添加 `[DONE]` 并更新完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 检查 `git status`、`git diff` 和最近提交，确认只提交本次任务相关变更；按仓库风格创建清晰提交。
8. 提交后停止，不推进下一个任务。

## 进度记录

- 初始化：已写入执行计划，下一步读取 `TODO.md` 确认首个未完成任务。
- 已确认首个未完成任务为 `P3-T02`。下一步查看 `PLAN.md` 相关章节和最近提交，仅确认是否存在直接相关前置问题。
- `PLAN.md` §6 要求 loader 覆盖 `lib` 无 main 成功、`bin` 无 main 失败；最近提交没有未完成事项说明。下一步检查 `cone/package.rs` 和 `frontend.rs` 的当前 entry 选择逻辑。
- 当前实现发现：`load_cone_source_package` 无条件要求 `src/main.scoop`，`ProjectInput` 的 explicit cone 也无条件保存 main index。将把 loader 的 entry anchor 改为 `bin` 专属，frontend 目录输入继续只接受 `bin` 作为 executable consumer。
- 已完成主要代码改动：`ConeSourcePackage.main` 改为 `Option<PathBuf>`，`bin` 才要求 main anchor；`lib/syslib` 可无 main 且 main 命名文件不再被当作入口。已添加 loader 单测和 entry selection 的非 consumer main 回归单测，下一步运行验证。
- 验证结果：`cargo fmt` 通过；`cargo test -p scoopc cone::package -- --nocapture` 通过；`cargo test -p scoopc frontend::tests -- --nocapture` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过；`cargo run -p scoop -- test` 通过（fixtures: ok，1562 checks）；最终 `cargo build` 通过。
- 已更新 `TODO.md`：任务索引和标题均将 `P3-T02` 标记为 `[DONE]`，并写入完成记录；下一步检查 git diff/status/log 并提交本任务。
