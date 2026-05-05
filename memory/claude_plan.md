# 当前执行计划

## 范围
- 只处理 `TODO.md` 索引指向的第一个未完成详细任务。
- 以对应 `TODO-Px.md` 详细文件为准；如索引与详细文件不一致，同步索引。
- 不做开放式历史问题扫查；只处理阻塞当前任务或与当前任务直接相关的问题。

## 步骤
1. 读取 `TODO.md`，按索引顺序定位需要检查的详细 `TODO-Px.md` 文件。
2. 在详细任务文件中找到第一个标题未带 `[DONE]` 的任务。
3. 阅读该任务要求、约束、依赖和验证方式，并检查最近提交是否指出与该任务直接相关的未完成问题。
4. 实现该任务；如发现必须先修复的具体阻塞项，则在对应详细 TODO 中加入最小前置任务并同步索引后停止。
5. 运行相关测试和必要的格式/检查命令，修复由当前任务引入或阻塞当前任务的问题。
6. 在详细 `TODO-Px.md` 中将完成的任务标题标记为 `[DONE]` 并更新完成记录；必要时同步 `TODO.md`。
7. 更新本计划文件记录关键进展。
8. 按要求提交本次所有相关更改，然后停止，不进入下一任务。

## 当前状态
- 已读取 `TODO.md` 与 `TODO-P7.md`。
- 第一个未完成详细任务是 `P7-T02`：更新默认主线切换后的 driver/fixture/test/docs 假设，并增加默认路径等价与 hidden fallback 守护。
- 最近提交为 `[P7-T01R] Review selector default flip`，与本任务的前置 review 对齐，没有发现需要先插入的新前置项。
- 已盘点 selector 用法：实现代码中没有 `default legacy` / `fallback legacy` / `retry legacy` 命中；仍需清理的主要是少量诊断与测试中“必须显式 `--effect-pipeline refactor`”的表达。
- 编辑计划：更新相关诊断和 CLI parse 测试；新增默认路径与显式 refactor 的黑盒等价测试，覆盖 `dump-mir`、`build --emit-llvm`、`run`、`test --fixtures`；新增默认 build 失败不自动回 legacy 的守护测试；补充设计文档中 P7 后默认命令应省略 selector 的说明。
- 已完成实现与验证：新增默认/显式 refactor 等价黑盒测试，更新诊断、CLI 测试、build fallback 守护和 P6->P7 文档说明；发现 self-contained handle build 已经支持 refactor lowering，因此将旧“应失败”测试改成“不含 legacy handler-stack/outcome”的正向守护。
- 已运行定向验证、P7 smoke、搜索守卫与 `cargo clippy --all-targets -- -D warnings`，均通过。
- 已将 `TODO-P7.md` 的 `P7-T02` 与 `TODO.md` 索引标记为 `[DONE]` 并填写完成记录。
- 已提交本次任务更改：`aa5e5157 [P7-T02] Guard default refactor pipeline assumptions`。
- 当前只剩测试生成的未跟踪构建目录 `crates/scoop/target/`，未纳入任务提交。
- 本次调用到此停止，不进入下一任务。
