# 执行计划记录

## 说明

按要求先记录执行计划，再开始读取仓库状态与任务列表。这里记录的是可审阅的执行摘要、步骤、假设和进度更新；不包含逐字的内部推理。

## 初始计划

1. 检查最新一次 Git 提交，确认是否提到了已知问题、后续修复项或未完成事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务现有规划、依赖关系和上下文。
4. 评估该任务规模：
   - 如果任务足够明确且可在本轮完成，直接实施。
   - 如果任务过大或存在前置缺口，先拆分任务并更新 `TODO.md` 与 `PLAN.md`，然后仅执行拆分后的第一个子任务。
5. 在实现前检查相关代码、测试和规范文件，确认正确实现边界，避免以变通方案掩盖缺口。
6. 实现当前目标任务。
7. 运行相关测试，并在必要时运行更广泛的验证（包括 `cargo fmt`、相关测试、必要时 `cargo clippy --all-targets -- -D warnings`）。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时按要求重排任务。
   - 在 `PLAN.md` 中同步当前状态、依赖、阻塞原因或拆分结果。
   - 持续更新本文件记录关键进展。
9. 检查工作区变更，确保未误改无关内容。
10. 使用清晰的提交信息提交本轮更改，然后停止，不继续处理下一个任务。

## 当前状态

- 计划文件已创建。
- 已检查最新提交：`a7e4633 Update plan`。提交说明本身未点名需要立即修复的单个遗留缺陷；提交内容主要更新 `ISSUES.md` 与 `run_agent.sh`。`ISSUES.md` 是项目问题清单，不构成“最新提交单独新增且必须先修完再继续”的单一阻塞项。
- 已读取 `TODO.md` / `PLAN.md`。
- 当前首个未完成任务 `T3010b2aR` 已完成。

## 本轮执行结果

1. 已完成 `T3010b2aR` 审查，并确认 `resume_path` 的生产消费仍位于 plan builder 的 `materialize_resume_fragments` 阶段，不在 emitter 中执行。
2. 审查中发现一个边界泄漏：`HandleStateOp::ResumeAfterSite` 仍把完整 `hir::Expr` 暴露给下游阶段。虽然 emitter 当时没有按 AST 形状回扫，但该边界不够严格。
3. 已修复上述问题：
   - `ResumeAfterSite` 改为只保留 `source_span` / `source_ty` 元数据。
   - 新增 `HandlePlanBuilder.resume_source_exprs`，只在 builder 内部按 `site_id` 保存原始表达式，供 resume-tail 改写阶段使用。
   - emitter 的 resume slot 回填逻辑改为只消费 `source_span`、synthetic slot 与 contract frame metadata。
4. 已更新 `TODO.md` 与 `PLAN.md`：
   - `T3010b2aR` 标记为完成。
   - 记录了 review 发现的问题、修复内容、验证命令与审查结论。
   - 当前下一任务为 `T3009a`。
5. 已完成验证：
   - `cargo check -p scoopc`
   - `cargo test -p scoopc source_plan_rewrites -- --nocapture`
   - `cargo test -p scoopc resume_path_is_preserved_from_plan_to_segments_to_unified_machine -- --nocapture`
   - `cargo test -p scoopc unified_state_machine_preserves_execution_payload_metadata -- --nocapture`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all`
   - `cargo fmt`

## 下一步

- 检查最终 diff 与任务状态。
- 以 `T3010b2aR` 对应的清晰提交信息提交本轮更改。
- 停止，不进入 `T3009a`。
