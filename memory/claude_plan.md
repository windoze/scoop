# 当前调用计划

## 范围

- 以 `TODO.md` 作为权威任务列表。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 先识别当前任务，不做开放式历史问题扫查。
- 不使用规避方案、不弱化行为；若发现阻塞当前任务的具体前置项，则写入 `TODO.md`、提交并停止。

## 执行步骤

1. 读取 `TODO.md`，按标题前缀识别第一个未完成任务。
2. 只检查最新提交信息中是否有与该任务直接相关的未完成事项。
3. 阅读任务正文、依赖、验证要求和引用文件。
4. 通过定向搜索和文件读取审计相关实现与测试。
5. 用最小且完整的 spec-correct 变更完成当前任务。
6. 按任务需要新增或更新聚焦测试/fixture。
7. 先运行任务指定验证和相关窄验证；必要时再运行更广验证。
8. 若验证暴露当前任务范围内的阻塞项，能修则修；否则在 `TODO.md` 写入最小前置任务，提交并停止。
9. 任务完成后，在 `TODO.md` / 对应 TODO 文件中将标题标为 `[DONE]` 并更新完成记录。
10. 关键进展或计划变化时更新本文件。
11. 检查 git status/diff/log 后，用带任务号的描述性提交提交本次变更。
12. 停止，不开始下一个任务。

## 进度记录

- 已在读取项目任务文件前初始化本次调用计划。
- 已选择第一个未完成任务：`TODO-5.md` 中的 `P5-T05`。
- 最新提交为 `[P5-T04R] Review LIR optimization family`；提交标题中没有直接相关的未完成事项。
- 当前任务计划：审计 P4/P5 output 嵌套与可变性边界，审计 LIR opt 是否隔离上游输入，同步 cleanup/README/TODO 文档，运行指定验证，然后只提交本任务。
- 审计结果：没有 `canonical_snapshot_mut()` 残留；`EffectFactsStageOutput` 只保存 effect facts；`LirStageOutput` 不保存上游 stage output wrapper；LIR opt 生产代码只消费 LIR-owned 输入。主要改动是文档/状态同步以及 TODO-6/P7 residual 边界注释更新。
- 验证已通过：`cargo fmt`、dependency gate、effect/lir facts 测试、effect_facts_stage 测试、effect_lowered 测试、`cargo clippy --all-targets -- -D warnings` 和 `git diff --check`。
- 已在 `TODO-5.md` 将 `P5-T05` 标为 `[DONE]`，并同步 `TODO.md`。
