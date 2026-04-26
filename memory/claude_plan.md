# 执行计划与进度记录

## 约束说明

- 按用户要求，先记录执行计划，再进行仓库检查与命令执行。
- 这里记录的是可审计的执行计划、依据与进度摘要，不包含逐字内部推理。
- 本轮目标：只完成 `TODO.md` 中第一个未完成任务；若发现前置缺陷，则优先修复前置缺陷或将其整理为新的前置任务后停止。

## 初始执行计划

1. 检查最新一次 Git 提交信息，确认是否明确提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务是否一致。
4. 评估该任务是否过大：
   - 若可直接完成，则开始实现。
   - 若过大，则先细化为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并将第一个子任务作为本轮目标。
5. 在实现前后检查相关代码、测试与规范上下文，识别任何已存在缺陷、回归、规范不匹配或实现边界问题。
6. 若发现阻塞性既有问题：
   - 先修复；
   - 或将其作为当前任务的前置任务插入 `TODO.md`，更新 `PLAN.md`，提交后停止。
7. 完成本轮目标后，运行相关验证：
   - 至少执行与改动直接相关的测试；
   - 若可行，执行更广的验证，包括 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`，以及任务相关命令。
8. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成；
   - 在 `PLAN.md` 中同步状态与后续影响；
   - 持续更新本文件中的进度记录。
9. 使用清晰的 Git 提交信息提交本轮改动，然后停止，不继续下一个任务。

## 进度记录

- 已完成：创建本计划文件并写入初始执行方案。
- 已完成：检查最新提交、`TODO.md` 与 `PLAN.md`，确认当前首个未完成任务为 `T5000e1R Review：确认 InstanceKey 与 dump-ir materializer 的边界正确`。
- 已完成：初步复核 `crates/scoopc/src/monomorph/{mod.rs,lower.rs}`、`crates/scoop/src/commands/dump_ir.rs`、`crates/scoopc/src/mir/{mod.rs,lower.rs,materialize.rs}`，确认：
  - `dump-ir` 主入口已经直接调用 `mir::materialize_for_dump(...)`；
  - `monomorph::lower_for_dump(...)` 只剩兼容薄包装；
  - `InstanceKey` 与 `MonomorphKey` 在数据结构语义上已经分层。
- 新发现的关键边界问题：
  - `crates/scoopc/src/mir/materialize.rs` 当前通过 `hir::lower_for_compilation_unit_multi_files_with_type_env(...)` 构造 typed HIR；
  - 该入口仍会启用 `collect_generic_fun_instantiations(...)` 的旧 HIR 实例化路径；
  - 这意味着 `dump-ir` 在进入 MIR materializer 之前，仍可能先在 HIR 层偷偷生成 standalone generic `::<...>` 实例；
  - 这与 `T5000e1R` 的验收条件“dump-ir 消费 generic MIR template，而不是继续把 HIR 重新 lower 成实例”直接冲突。
- 计划调整：
  1. 先修改 HIR lowering API，提供一个显式“只生成 generic typed HIR template、不做 standalone generic HIR 实例化”的编译单元入口。
  2. 让 `mir::materialize_for_dump(...)` 改用该入口，确保实例化唯一发生在 MIR materializer 内部。
  3. 补两类回归测试：
     - 锁定 `materialize_for_dump` 的 generic MIR template 不含预先生成的 standalone `::<...>` HIR/MIR 实例；
     - 锁定 per-`InstanceKey` cache 会对重复实例请求去重。
  4. 运行相关测试与全量校验；若无新的前置缺陷，再更新 `TODO.md` / `PLAN.md` 并提交。
- 已完成：实现上述 API/调用链调整。
  - 在 `crates/scoopc/src/hir/lower/mod.rs` 新增 generic-only 的 typed compilation-unit lowering 入口，并把内部 lowering 选项细化为“standalone generic fun 实例化”和“owner-specialized member fun 实例化”两个开关；
  - `lower_typed_for_dump(...)` 与 `mir::materialize_for_dump(...)` 现都显式关闭这两条旧 HIR 实例化路径；
  - build / frontend 继续沿用原有会物化实例的 lowering 入口，没有改动编译单元主路径行为。
- 已完成：新增并通过两条回归测试。
  - `generic_mir_template_for_dump_stays_free_of_hir_level_instances`
  - `materialize_for_dump_dedups_repeated_instance_requests`
- 已完成：补跑既有 monomorph/materialize/dump-mir 回归，并完成全量校验。
  - 通过的关键验证包括：
    - `cargo test -p scoopc monomorph::lower -- --nocapture`
    - `cargo test -p scoopc mir::tests::dump_mir_keeps_generic_functions_as_templates_before_monomorphization -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_handles_type_body_generic_member_fun_roots -- --nocapture`
    - `cargo test -p scoopc materialize_for_dump_distinguishes_companion_member_fun_effect_instances -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
- 已完成：更新 `TODO.md` 与 `PLAN.md`，将 `T5000e1R` 标记为完成，并记录本轮 review 中发现并修复的真实边界问题。
- 下一步：检查工作区改动并提交 Git，然后停止，不进入 `T5000e2`。
