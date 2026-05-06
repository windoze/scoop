# 当前执行计划

## 约束

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若发现当前任务被具体前置缺陷阻塞，优先在 `TODO.md` 中插入最小前置任务并提交，不绕过实现。
- 完成后更新 `TODO.md` 的任务标题与完成记录，必要时才更新 `PLAN.md`。
- 验证相关测试，最后提交本轮全部相关变更。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务及其验收要求。
2. 检查最近提交是否明确提到与该任务直接相关的未完成事项。
3. 阅读与当前任务相关的代码、测试和文档，确认最小实现范围。
4. 实现当前任务；如果出现阻塞性缺口，按规则更新 `TODO.md` 并停止。
5. 添加或调整覆盖当前行为的测试/fixture。
6. 运行任务要求的验证命令和必要的回归测试。
7. 更新 `TODO.md` 的 `[DONE]` 标记与完成记录；仅当阶段计划变化时更新 `PLAN.md`。
8. 提交所有本轮相关变更并停止。

## 进度

- 已创建本轮初始计划。
- 已读取 `TODO.md`，第一个未完成任务为 `MIR-T01：落地 refactor production MIR strict verifier`。
- 已检查最近提交：`[MIR-T00] Add MIR placeholder inventory`，未发现需要优先处理的直接未完成事项。
- 已阅读 MIR 数据结构、现有 `validate_refactor_direct_style`、refactor `mir_stage::run` 和 placeholder inventory。
- 当前编辑计划：在 `mir/mod.rs` 增加 `File::validate_refactor_production` 与生产级 placeholder/return/site metadata 规则；在 `mir_stage::run` 中替换为 strict verifier；补充 `refactor_mir_no_todo_*` 单测。
- 已实现 strict production verifier、stage 接入和 `refactor_mir_no_todo_*` 单测，并已运行 `cargo fmt`。
- 已运行 `cargo test -p scoopc --no-default-features refactor_mir_no_todo`，8 个相关测试通过。
- 已补跑 `cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`，inventory 回归通过。
- `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` 首次发现 `validate_refactor_production_perform` 参数过多；已用 site context 合并参数并重新格式化。
- 已重新运行 `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`，通过。
- 已重新运行 `cargo test -p scoopc --no-default-features refactor_mir_no_todo`，通过。
- 已更新 `TODO.md`，将 `MIR-T01` 标记为 `[DONE]` 并补充完成记录。
- 下一步检查 git diff/status，然后提交本轮变更并停止。
