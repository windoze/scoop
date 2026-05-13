## 当前目标

执行 `TODO.md` 中第一个未完成任务 `P7-T01R`：复核全量 stable-id 收口结果，确认方案闭合、外部 surface 已脱离 dense id/path/raw Debug/pretty text 控制，且没有引入功能漂移；若发现真实阻塞，则把最小前置任务写回 `TODO.md`、提交后停止。

## 约束说明

- `TODO.md` 是任务顺序与完成状态的唯一事实来源。
- 只有标题显式带有 `[DONE]` 的任务才算完成。
- 默认不拆分当前任务；仅在存在具体且未跟踪的前置阻塞时，才引入最小新前置任务。
- 不以变通方案、夹具特判、缩小范围等方式绕过规范缺口。
- 本次只完成一个任务，然后停止。

## 执行计划

1. 读取 `TODO.md`，识别第一个未完成任务。
2. 检查最近提交信息，确认是否存在与该任务直接相关且未完成的问题；若有且会阻塞当前任务，则把它并入当前任务或作为前置写入 `TODO.md`。
3. 阅读 `P7-T01` 至 `P7-T01D` 完成记录，以及 `PLAN.md` §5/§6、`STABLE_ID.md` §10/§11/§12，整理本次 review 的 8 条签收标准。
4. 复核当前代码与审计入口，必要时补跑定向与全量验证，确认 external surface、private linkage、ABI mangling、dump/RTTI/JSON/object surface 与语义回归状态。
5. 若验证通过，则完成 `P7-T01R`：
   - 将已完成任务标题改为 `[DONE]`。
   - 补全完成记录。
   - 仅在阶段计划变化时更新 `PLAN.md`。
6. 若发现真实阻塞，则保持 `P7-T01R` 未完成，在 `TODO.md` 中增加最小前置任务并调整依赖顺序。
7. 提交所有本次相关修改，提交信息以任务 ID 开头。
8. 若本次完成后 `TODO.md` 中已无未完成任务，则创建发布标签 `v0.1.0`。
9. 停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件。
- 已确认第一个未完成任务为 `P7-T01R`。
- 最近提交为 `P7-T01D`，内容是补齐 LLVM stable-id 的 type-param key 传递，并明确说明 `P7-T01R` 可继续执行；暂未发现需先插入的新前置任务。
- 已确认这是 `TODO.md` 中最后一个未完成任务；若签收通过，需在提交后创建 `v0.1.0` 标签。
- 已完成基线复核：读取 `PLAN.md` §5/§6、`STABLE_ID.md` §10/§11/§12，以及 `P7-T01` 到 `P7-T01D` 完成记录，整理出最终 8 条签收标准。
- 已完成最终验证矩阵：
  - `cargo test -p scoopc stable_id_audit_grep_inventory_scans_repo_roots -- --nocapture`
  - `cargo test -p scoopc checkout_root -- --nocapture`
  - `cargo test -p scoopc distinct_virtual_cones -- --nocapture`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop_tools -- spec-fixtures check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 验证结果：全部通过；未发现需要新增到 `TODO.md` 的 blocker。
- 已在 `TODO.md` 中把 `P7-T01R` 标记为 `[DONE]`，并补齐最终签收记录。
- 下一步：检查工作树、提交本次变更，并在提交后创建 `v0.1.0` 标签。
