# Claude 执行计划

说明：本文件记录本次调用的可审计执行计划、关键进展和验证结果；不记录隐藏推理过程。

## 当前任务

- 按 `TODO.md` 的权威顺序，当前第一个未完成任务是 `P7-0-T01：引入稳定 ID 与 retired ledger`。
- 最新提交 `454c1c7f Update plan` 未明确提到与当前任务直接相关的未完成阻塞项。
- 本次只完成 P7-0-T01；完成后提交并停止，不进入 P7-0-T02 或后续退场任务。

## 执行计划

1. 读取 `PLAN.md` P7-0 相关段落、现有 audit inventory/CLI 代码、当前 inventory schema 与生成数据格式。
2. 保留当前 1,284 行 immutable baseline 到 `audit/UMB_inventory_initial.csv`，新增空 retired ledger `audit/UMB_retired.csv`。
3. 改造 audit inventory 逻辑：active inventory 从 initial/上一版 CSV 继承稳定 ID，支持 line drift 匹配，并校验 active/retired ID 互斥且并集等于 initial ID 集。
4. 补充测试覆盖“模拟删除 row 后 remaining IDs 不重排”和歧义匹配报错。
5. 运行 P7-0-T01 指定验证：`cargo run -p scoopc --bin umb-audit -- diff`、`cargo run -p scoopc --bin umb-audit -- stats`、`cargo test -p scoopc audit:: -- --nocapture`。
6. 若验证通过，更新 `TODO.md` 将 P7-0-T01 标为 `[DONE]` 并填写完成记录；`PLAN.md` 仅在阶段计划改变时更新。
7. 更新本文件记录关键进展和验证结果。
8. 提交所有本次任务相关变更，随后停止。

## 进展记录

- 已写入本次调用的初始执行计划。
- 已确认当前任务为 `P7-0-T01：引入稳定 ID 与 retired ledger`。
- 已读取 `PLAN.md` P7-0-T01、现有 audit inventory 代码、CLI 和 schema。
- 已新增 immutable baseline 文件 `audit/UMB_inventory_initial.csv`（从当前 1,284 行 active inventory 复制）与空 retired ledger `audit/UMB_retired.csv`。
- 已改造 `crates/scoopc/src/audit/umb_inventory.rs`：当前扫描结果从 initial baseline 继承 stable ID，支持 exact/unique/order line drift 匹配，并校验 active/retired/initial ID 分区。
- 已新增测试覆盖模拟删除 row 后 remaining IDs 不重排、line drift 顺序配对和歧义匹配报错。
- 已更新 `audit/UMB_inventory_schema.md` 记录 initial snapshot、retired ledger 和 stable ID 匹配规则。
- 已完成验证：`cargo run -p scoopc --bin umb-audit -- diff` 通过（in sync，1284 entries）；`cargo run -p scoopc --bin umb-audit -- stats` 通过（total_entries=1284）；`cargo test -p scoopc audit:: -- --nocapture` 通过（20 passed）；`cargo clippy --all-targets -- -D warnings` 通过。
- 已更新 `TODO.md`：顶部状态改为 P7-0-T01 已完成，并将 `P7-0-T01` 标为 `[DONE]`、填写完成记录。
- 剩余步骤：检查 git diff/status，提交本任务相关变更，然后停止。
