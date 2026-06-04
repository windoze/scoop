# 执行计划

## 约束
- 以 `TODO.md` 为任务排序和完成状态的唯一来源。
- 本次只完成第一个未标记 `[DONE]` 的任务，然后停止。
- 若遇到阻塞当前任务的未排期缺陷，先修复或在 `TODO.md` 中新增最小前置任务并提交。
- 不使用规避方案；若实现与规格不匹配，修复根因或显式排期。

## 初始步骤
1. 读取 `TODO.md`，识别第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的要求、依赖、验证命令与完成记录格式。
3. 如最新提交明确提到与该任务直接相关的未完成事项，纳入当前任务范围或加入 `TODO.md` 前置项。

## 执行步骤
1. 根据当前任务读取相关代码、测试、规格与历史上下文。
2. 以最小正确改动实现任务，不修改无关用户变更。
3. 添加或更新覆盖任务行为的测试/fixture。
4. 按要求运行格式化、lint、测试与 fixture 验证。
5. 若验证失败，修复失败；若失败属于未排期且不能在当前任务中修复，则在 `TODO.md` 中加入正确顺序的任务并停止。
6. 完成后把当前任务标题标记为 `[DONE]`，更新完成记录。
7. 提交本次任务相关全部改动并停止。

## 当前进度
- 已读取 `TODO.md`，第一个未完成任务是 `T2-08A：补齐 LIR executable body 容器前置`。
- 已检查工作树与最新提交：最新提交 `[T2-08] Add executable body prerequisite` 直接对应当前前置任务，当前未提交改动仅有本计划文件。
- 已读取 LIR IR 与指令定义：现有 `instruction.rs` 只有 `LirStateBody`，`ir.rs` 仍保留 `LateLoweredSourceCallable` / `LateLoweredSourceBody` MIR alias、plain body slice 和 source-slice based classification/consumption。
- 实施策略：先新增 LIR-owned executable body 类型与 anchor，选择统一 state-owned body 表示，覆盖 plain callable header/param/local/body 容器；本任务不删除旧 MIR overlay，也不改 lowering/codegen 消费路径，留给依赖的 `T2-08`。
- 已实现并验证：新增 executable body/header/param/local/state/anchor 类型和单测，`T2-08A` 已在 `TODO.md` 标记 `[DONE]`。
- 已通过验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
- 下一步检查 diff/status，提交本次任务改动后停止。
