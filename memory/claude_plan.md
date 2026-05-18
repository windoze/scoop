# 当前执行计划

## 约束说明
- 仅记录可审阅的执行计划与进展摘要，不写入隐藏推理链。
- 本轮只完成 `TODO.md` 中第一个标题未带 `[DONE]` 的任务，完成后停止。

## 初始步骤
1. 阅读 `TODO.md`，按文件顺序定位第一个未完成任务。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；只有直接阻塞当前任务时才纳入范围。
3. 阅读当前任务关联的代码、测试、规范或 fixture，确认验收条件与依赖。

## 执行步骤
1. 按当前任务要求做最小但完整的实现或修复。
2. 若发现 spec 不匹配、缺失语言特性或真实阻塞项，不做 workaround；在 `TODO.md` 中插入最小必要前置任务，保留当前任务未完成，提交后停止。
3. 为实现补充或调整最相关的测试/fixture。
4. 运行当前任务要求的验证命令；必要时运行更窄范围的回归测试，再视情况运行更广范围测试。
5. 修复验证中发现的与当前任务直接相关的问题。

## 收尾步骤
1. 将已完成任务标题加上 `[DONE]`，并更新其完成记录。
2. 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
3. 检查 git 状态和 diff，提交本轮所有相关变更。
4. 停止，不继续处理下一个任务。

## 当前状态
- 已读取 `TODO.md`，第一个未完成任务是 `U5-T03：bucket-driven 直接对账 fixture`。
- 最新提交为 `[U5-T02] Add UMB spec fixture corpus`，未声明新的未完成阻塞项；其输出是本任务直接输入。
- 已检查当前覆盖：139 个 fixture 的 `COVERS` 与 `_index.csv` 一致；1213/1284 个 UMB id 已由 fixture 覆盖，缺口是 B-01 的 71 个 helper invariant id，需要 sentinel 记录而非用户 fixture。
- 还发现 B-07 与 B-23 缺少同 bucket 专属 ID 覆盖；B-23 与 B-27 strategy 明确要求 U5-T03 补专属 fixture。
- 当前编辑计划：补 B-01 README/category/strategy 的 sentinel coverage；更新 B-07 正/负 fixture 的 `COVERS` 与 index；新增 B-23 positive fixture、修正 B-23 negative fixture覆盖；新增 B-27 negative fixture；回填 `_index.csv`、相关 bucket md、strategy md 与 spec matrix。
- 已完成编辑：新增 B-23 positive、B-27 negative；更新 B-07/B-23 direct `COVERS`；B-01 记录 sentinel-only coverage；`_index.csv`、bucket docs、strategy docs、spec matrix 已回填。
- 自定义结构校验通过：141 个 fixture 与 141 条 index row 同步，fixture 覆盖 1213 个 UMB id，B-01 sentinel 覆盖 71 个 UMB id，合计覆盖 1284/1284；每个 bucket 都有同 bucket direct fixture 或 sentinel 指针。
- 下一步运行验证：`cargo run -p scoop -- test tests/fixtures/umb_fix/`、`cargo test -p scoopc audit::spec_coverage -- --nocapture`、`cargo run -p scoopc --bin umb-audit -- stats`、`cargo run -p scoopc --bin umb-audit -- diff`、`cargo clippy --all-targets -- -D warnings`，必要时补充 `cargo test --all --all-targets`。
- 验证完成：`cargo run -p scoop -- test tests/fixtures/umb_fix/` 通过（141 fixtures skipped/pass）；`cargo test -p scoopc audit::spec_coverage -- --nocapture` 通过（当前无匹配测试）；`umb-audit stats` / `diff` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（scoopc 874 passed，umb-audit 3 passed，其他目标通过）。
- 下一步更新 `TODO.md`，将 U5-T03 标记 `[DONE]` 并写入完成记录，然后检查 diff 并提交。
