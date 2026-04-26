# 执行计划（决策摘要）

说明：
- 用户要求先把“完整思考过程”和执行计划写入本文件；这里记录的是可审计的执行计划、假设、检查点与决策摘要，不写入逐字内部推理。
- 本轮目标是：先检查最新提交是否提到既有问题并优先修复；然后读取 `TODO.md` 找到第一项未完成任务；若任务过大则拆分并更新 `PLAN.md`/`TODO.md`；完成恰好一个任务后测试、更新文档、提交并停止。

初始步骤：
1. 检查仓库状态与最新提交信息，确认是否存在提交中明确提及、且尚未修复的既有问题。
2. 读取 `TODO.md` 与 `PLAN.md`，确定第一项未完成任务及其上下文。
3. 结合代码现状判断任务是否可直接完成；若过大，则先拆分任务并更新计划文件。
4. 实施该任务所需的最小正确修改，不接受规避缺陷的变通方案。
5. 运行相关测试、格式化、lint；若暴露既有问题，则先修复问题或把前置任务插入 `TODO.md` 并停止。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况与任何计划调整。
7. 提交 git commit，然后停止，不继续处理下一项任务。

执行约束：
- 若在探查、测试、评审中发现任何既有 bug / 回归 / 规范不匹配 / 未完成实现边界，必须立即优先处理，或在 `TODO.md` 中插入前置任务后停止。
- 不回退用户已有修改；不使用破坏性 git 命令。
- 所有输出与记录使用中文。

待检查项：
- 最新 commit message / diff 是否包含待修复问题提示。
- `TODO.md` 第一项未完成任务是什么。
- 是否存在阻塞该任务的已知实现缺口。

进度日志：
- 2026-04-27：已创建计划文件，下一步开始检查最新提交与任务列表。
- 2026-04-27：已确认最新提交未直接声明待修复 issue；`TODO.md` 第一项未完成任务为 `T5000fR Review：确认 summary 已按单态实例而不是按函数名工作`。
- 2026-04-27：review 过程中发现一个必须先修的既有问题：
  - `MaterializedMirSummaries` 对外虽然按 `InstanceKey` 暴露，但 `crates/scoopc/src/mir/materialize.rs` 中 `instance_fqn()` 目前只用 `template.fqn + type_args + eff_args` 生成 materialized root identity；
  - 仓库支持同名 overload，而 `TemplateKey` 之所以携带 `source_path + decl_span`，就是为了区分这些模板；
  - 因此，若存在“同名 generic overload + 相同实例化实参”的情况，不同模板实例会落到同一个 materialized root 字符串，进而让 `crates/scoopc/src/mir/summary.rs` 中按 `root_fqn: String` 建图的 pending summaries、direct-call graph、SCC 与 outward-effect 传播发生碰撞。
- 2026-04-27：计划调整：
  1. 先为 overloaded generic template 引入稳定且唯一的 materialized root symbol disambiguator，确保实例投影到 MIR family symbol 时仍保持单射。
  2. 补 materialize / summary 回归测试，覆盖“同名 overload + 相同 type args 不得共享 root identity / summary”的场景。
  3. 跑相关测试与全量校验。
  4. 若验证通过，再更新 `TODO.md` / `PLAN.md` 完成 `T5000fR` 并提交。
- 2026-04-27：已完成代码修复与最小回归：
  - 在 `crates/scoopc/src/mir/materialize.rs` 中新增 canonical template → stable overload suffix 的映射；当同一 `template.fqn` 存在多个 canonical overload 时，materialized root 会在 `::<args>` 之后追加稳定的 overload discriminator，而不是继续只用 `fqn::<args>`；
  - `instance_fqn()` 已统一消费这层 suffix，因此 family root、nested lambda rewrite、direct-call rewrite、summary root 映射都会拿到不冲突且保留原始 `template_fqn::<...>` 前缀的实例化符号；
  - 在 `crates/scoopc/src/mir/summary.rs` 新增回归测试 `overloaded_generic_instances_keep_distinct_summary_identity`，验证两个同名 generic overload 在相同 `Int` 实例化下仍保留不同 root symbol，并分别得到 `Param(0)` / `Param(1)` 的 summary；
  - 已通过最小验证：
    - `cargo test -p scoopc overloaded_generic_instances_keep_distinct_summary_identity -- --nocapture`
    - `cargo test -p scoopc summaries_are_keyed_by_instance_identity -- --nocapture`
- 2026-04-27：下一步执行全量格式化、测试与 clippy，然后回写 `TODO.md` / `PLAN.md` 完成 `T5000fR`。
- 2026-04-27：全量验证已完成：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 全部通过。
- 2026-04-27：已回写 `TODO.md` / `PLAN.md`：
  - 将 `T5000fR` 标记为完成，并记录 review 中发现并修复的 overload-aware instance identity 问题；
  - 明确下一条待执行任务切换为 `T5000g 在 MIR 层实现通用 devirtualization`。
