# 本次执行计划

1. 读取 `TODO.md`，确认第一个未标记为 `[DONE]` 的任务。
2. 检查最近一次提交是否存在与该任务直接相关且尚未完成的问题；若存在，将其视为当前任务的一部分或作为前置任务写回 `TODO.md`。
3. 阅读与当前任务相关的代码、测试、规格与任务说明，明确实现边界与验证要求。
4. 直接完成当前任务；如果遇到会阻塞任务的真实缺陷或缺失能力，则先按要求更新 `TODO.md` / `PLAN.md` 并停止，不采用变通方案。
5. 运行与当前任务相关的验证命令，并在必要时修复失败。
6. 更新 `memory/claude_plan.md` 记录关键进展，更新 `TODO.md` 的完成状态与 completion record；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展记录

- 已创建初始计划文件，下一步读取 `TODO.md` 与最近提交信息以确认当前任务。
- 已确认首个未完成任务为 `P4-T01`：重写 overload / template / instance 的 exported identity 来源。
- 已检查最新提交 `[P3-T02R]`；提交信息未显式引入与 `P4-T01` 直接相关的新未完成前置问题，因此继续按 `P4-T01` 本体推进。
- 下一步：读取 `mir/materialize.rs`、`hir/lower/util.rs`、`stable_id.rs` 及相关测试，确认当前 exported naming 仍通过哪些旧路径依赖 `source_path + decl_span`、pretty text 与 `TypeId`。
- 已确认当前缺口不在 `stable_template_symbol_suffix()` 主实现本身，而在后续阶段对 `InstanceKey -> StableInstanceKey` 的“现场重建”：部分 late-lowered / LLVM 路径仍只拿 `template.fqn + type/effect args` 重建 key，遗漏 overload signature，导致同名 overload 的同型实例可能坍缩到同一 stable key。
- 已开始修复方案：
  1. 在 `MaterializedMir` 上持久化 authoritative `StableInstanceKey` side table；
  2. 在 late-lowered callable 上显式携带该 key，禁止 downstream 再按旧字段重建；
  3. 用该 authoritative key 替换 LLVM closure stable key 与 effect stable naming 的重建路径；
  4. 补定向测试覆盖 overloaded generic 同型实例的 distinct/path-stable exported identity。
- 已完成实现：
  - `MaterializedMir` 现在保存 authoritative stable instance/template/signature side table，并通过 `instance_exported_fun_symbol(...)` 暴露 exported symbol 路径。
  - effect facts 的 `ConcreteOpKey`、late-lowered callable、LLVM materialized-closure stable key、effect-lowered stable naming 已全部改为消费 authoritative stable key，而不是现场从 `InstanceKey` / `template.fqn` 重建。
  - 新增/更新回归测试，覆盖 overloaded generic exported symbol path-stability、downstream callable version key distinctness，以及 receiver overload direct-call 目标的 distinct overload-aware symbol。
- 验证已完成：
  - `cargo fmt`
  - 关键定向测试全部通过。
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
  - grep 审计已确认 exported naming 路径不再回退到 `source_path + decl_span` / pretty-text 驱动。
- 下一步：查看工作区变更、回写 `git` 提交信息，并按 `P4-T01` 提交后停止。
