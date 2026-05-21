执行计划记录

当前目标：按照 TODO.md 的顺序完成第一个未标记为 [DONE] 的任务，然后停止。

约束与原则：
- TODO.md 是任务顺序、要求、依赖和完成状态的唯一权威来源。
- 只有标题显式带有 [DONE] 的任务才视为完成。
- 本次只处理第一个未完成任务，不做开放式历史问题清扫。
- 如果当前任务被真实前置缺口阻塞，将在 TODO.md 中添加最小必要前置任务并停止。
- 不使用规避方案、夹具特例或弱化规格的方式推进任务。
- 完成实现后运行相关验证；若验证暴露当前任务范围内的问题，立即修复。
- 任务完成后更新 TODO.md 的标题和完成记录，必要时才更新 PLAN.md。
- 最后提交本次任务相关变更，提交前检查状态和差异。

步骤：
1. 读取 TODO.md，定位第一个标题未带 [DONE] 的任务。
2. 查看该任务的要求、依赖、验证项和完成记录；必要时查看最新提交是否提到与该任务直接相关的未完成事项。
3. 读取任务涉及的代码、测试、文档和夹具，确定最小正确实现范围。
4. 实施任务所需修改，优先采用小而正确的补丁。
5. 运行任务指定或最相关的测试/检查，修复当前任务范围内失败。
6. 更新 TODO.md，将已完成任务标题加上 [DONE] 并补全完成记录。
7. 运行提交前检查：git status、git diff、近期日志。
8. 提交所有本任务相关未提交文件，然后停止。

进度：
- 已读取 TODO.md；第一个未完成任务是 P1-T06（P1 全包清场、文档同步与依赖审计）。
- 下一步读取 TODO-2.md 中 P1-T06 的详细要求和验证项。
- 已读取 TODO-2.md 的 P1-T06 任务详情；任务要求是全仓清理 P1 旧 owner/误导性 compilation-unit 文字、审计基础 crate 依赖、更新 active 文档和 TODO 状态。
- 已检查最新提交：`ca275c86 [P1-T05R] Review cone compilation unit API`，未看到直接声明的未完成问题；当前工作区除本计划文件外无其它未提交变更。
- 下一步执行限定范围搜索：旧基础模块路径、resolver cone identity 旧路径、compilation unit 误导性文字和 active docs 中的 P1 描述。
- 已完成第一轮清场编辑：更新 README、active pipeline docs、`scoopc` facade/adapters 和基础 crate 注释，说明 legacy adapter 保留原因，并移除 P1 前状态的过期描述。
- 下一步复查关键词命中、依赖树和格式，再运行 P1-T06 指定验证。
- 已通过验证：`cargo fmt`、`cargo fmt --check`、`cargo test --all --all-targets --no-default-features`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo clippy --all-targets -- -D warnings`、`dependency-gate`、5 个基础 crate 的 `cargo tree`、关键词搜索和 `git diff --check`。
- 已更新 TODO.md 与 TODO-2.md，将 P1-T06 标记为 [DONE] 并写入完成记录。
- 下一步检查最终 diff/status/log，提交本任务相关变更后停止。
