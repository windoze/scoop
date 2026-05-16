# 执行计划

说明：我会记录可审查的执行计划、关键决策、进度和验证结果；不会记录私有逐步推理细节。

## 当前目标

- 按 `TODO.md` 的顺序识别第一个标题未带 `[DONE]` 的任务。
- 完成且只完成该任务；若发现该任务被具体前置缺陷阻塞，则只新增最小必要前置任务并停止。
- 完成后更新 `TODO.md` 的 `[DONE]` 标记和完成记录，必要时仅在阶段计划变化时更新 `PLAN.md`。
- 运行相关验证，修复当前任务范围内的问题。
- 提交包含本次任务全部相关变更的 Git commit，然后停止。

## 步骤计划

1. 读取 `TODO.md`，确定第一个未完成任务及其依赖、验证要求和完成记录格式。
2. 检查最新提交是否明确提到与该任务直接相关的未完成事项；如有，将其纳入当前任务或作为前置任务记录。
3. 阅读当前任务涉及的代码、测试、规格或夹具，确定最小正确实现范围。
4. 实施任务要求，避免规避规格或夹具私有 hack。
5. 添加或更新最小相关测试/fixture。
6. 运行任务要求的验证命令以及必要的补充验证。
7. 根据验证结果修复问题，直到当前任务范围内验证通过或确认存在必须先处理的阻塞项。
8. 更新 `TODO.md`：完成则给任务标题加 `[DONE]` 并填写完成记录；阻塞则新增最小前置任务并保留当前任务未完成。
9. 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
10. 提交本次相关变更，提交信息包含任务编号和简要目的。

## 进度日志

- 已创建本执行计划；下一步读取 `TODO.md` 以定位当前任务。
- 已定位第一个未完成任务：`P0-T01`（冻结 reshape baseline 与 fixture 三分类清单）。最新提交是计划/TODO 初始化提交，未明确提到与该任务直接相关的未完成缺陷。
- 已运行全量 fixture baseline：`cargo run -p scoop -- test` 通过，生成 `target/reshape-baseline/baseline-pass.txt`（1330 个 pass target，raw summary 为 1367 checks）。
- 已生成 `stdlib-fixtures.txt`：21 条分类，KEEP-RENAME 8，MERGE-INTO 0，DELETE 13；已确认 14 个 `stdlib_*.scoop` 全覆盖。
- 已生成 `fstring-fixtures.txt`：61 个实际 f-string fixture 文件；spot check 覆盖单 expr、多 expr、Bool、Int、Char、Float、String、raw f-string。当前 baseline 没有 `{{` / `}}` 转义 fixture，P6-T01 已要求新增含转义的 owner fixture。
- 已在 `TODO.md` 与 `TODO-1.md` 标记 `P0-T01` 为 `[DONE]`，并写入完成记录、验证结果和后续 P6 接手的 f-string 转义覆盖缺口。
- 已运行 `cargo clippy --all-targets -- -D warnings`，通过。
