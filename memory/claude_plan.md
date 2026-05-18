执行计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未带 `[DONE]` 的任务；不进行开放式历史问题排查。
2. 读取该任务相关上下文，并按需查看 `PLAN.md`、最近提交和相关源码，确认任务范围、依赖和验证要求。
3. 如果发现当前任务被具体前置问题阻塞，只在 `TODO.md` 中添加最小必要前置任务或依赖记录，提交后停止。
4. 如果任务可直接执行，实施最小正确改动，避免规避规格或夹带无关重构。
5. 运行当前任务要求的验证命令，并补充必要的针对性测试；若失败，修复同一根因影响的相关问题。
6. 将当前任务标题改为 `[DONE] ...`，更新完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 提交本次任务涉及的全部变更，提交信息使用任务编号开头。
8. 停止，不继续处理下一个任务。

进度记录

- 已写入初始执行计划；下一步读取 `TODO.md` 定位第一个未完成任务。
- 已读取 `TODO.md`，第一个未完成任务为 `U2-T02：36 份 bucket md 主体`。
- 本次执行范围固定为补齐 `audit/UMB_categories/B-01.md` 到 `B-36.md` 的主体内容、验证数字对账、更新 `TODO.md` 完成记录并提交；不处理 U3 及后续任务。
- 已检查最近提交 `3bfd5b5a [U2-T01] Record completion progress`，没有直接声明未完成且阻塞 U2-T02 的 issue。
- 已确认 inventory stats：1,284 entries、36 个 bucket 全部非空、missing spec/upstream gate 均为 0；当前 bucket 文档仍是 U2-T01 skeleton。
- 正在批量生成 U2-T02 文档主体：每份 bucket md 将包含完整 inventory symptom 表、3 个源码片段、root-cause hypothesis、spec linkage、post-fix class 分布、fixture pointer 与 open questions。
- 已生成 36 份 bucket 文档主体，并移除 skeleton 占位文字；下一步做结构和数字对账。
- 文档结构对账通过：36 份文档、1,284 条 inventory entry、每份 3 个 source excerpt、cross-class entry split 均匹配。
- 验证通过：`cargo run -p scoopc --bin umb-audit -- stats`、`cargo run -p scoopc --bin umb-audit -- diff`、`cargo test -p scoopc audit::umb_inventory -- --nocapture`、`cargo clippy --all-targets -- -D warnings`。
- 已更新 `TODO.md`：`U2-T02` 标记为 `[DONE]`，完成记录写入验证结果和对账摘要，顶部状态推进到 `U3-T01`。
