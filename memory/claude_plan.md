执行计划

1. 读取 TODO.md，按文档顺序定位第一个标题未带 [DONE] 的任务。
2. 阅读该任务的完整要求、依赖、验证要求和完成记录；必要时查看 PLAN.md 和最近提交，只限于判断当前任务是否被最新未完成事项影响。
3. 检查工作区状态，避免覆盖用户或其他代理的未提交改动。
4. 根据当前任务定位相关代码、测试和 fixtures，实施最小且完整的修复或功能变更。
5. 按要求运行格式化、lint、相关测试，并在需要时运行完整测试套件和 fixture 套件。
6. 如果遇到阻塞的缺失功能、规格不符或未安排的失败测试，更新 TODO.md 插入最小 prerequisite 任务，保持当前任务未完成，提交后停止。
7. 如果任务完成，更新 TODO.md：在任务标题前加 [DONE] 并填写完成记录；仅在阶段计划实际变化时更新 PLAN.md。
8. 提交所有本次任务相关改动，然后停止，不继续下一个任务。

当前状态

- 已读取 TODO.md，首个未完成任务是 `T2-05-R：Review T2-05`。
- 本次 invocation 只执行该 review 任务，完成后停止。
- Review 发现 `LirFacts` 顶层 site/dispatch 平表已删除，但 verifier 仍未拒绝 plain dynamic/dispatch site 缺少节点内 contract、dispatch 空候选和 embedded owner/source 漂移。
- 已开始在 `scoopc_lir_facts` verifier 中补齐这些结构校验，并新增对应单测。
- 已补齐 verifier 校验，并修正 `lir_facts_builder` boundary 分支避免重复挂载已发布的 dynamic/dispatch payload。
- 快速验证已通过：`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc effect_lowered_program -- --nocapture`。
- 首次 `cargo clippy --all-targets -- -D warnings` 发现 helper 显式 lifetime 可省略；已修复并重新格式化，接下来重跑 clippy。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo test --all --all-targets` 已通过。
- `cargo build -p scoop -p scoopc` 已通过。
- `python3 tools/dependency_gate.py` 已通过；`python3 tools/spec_fixtures.py check` 已通过。
- `python3 tools/run_fixtures.py` 已通过。
- 下一步更新 TODO.md，将 `T2-05-R` 标记为完成并记录 review 修复与完整验证命令。
- TODO.md 已将 `T2-05-R` 标记为 `[DONE]` 并写入完成记录。此后仅修改了文档/进度记录，不需要重跑完整测试。

T2-05-R 执行步骤

1. 检查 git 状态和最近提交，确认是否有与 T2-05-R 直接相关的未完成事项或已有未提交改动。
2. 审查 T2-05 相关实现：确认 site 数据归 site/control 节点所有，`source_call_sites`、`class_ctor_call_sites`、`reflection_call_sites`、`dynamic_invokes`、`dispatches` 顶层 facts 与公开复合 key 已消失。
3. 搜索相关 `.get(key)` / `(owner_callable, site_id)` / `BuildCallSiteKey` 使用点，区分允许的构造期私有去重与禁止的消费期 lookup。
4. 如发现 review 问题，实施最小正确修复并补充测试；如发现阻塞性规格缺口，更新 TODO.md 插入 prerequisite 并停止。
5. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`，然后运行完整基线命令。
6. 更新 TODO.md，将 `T2-05-R` 标记为 `[DONE]` 并写入 completion record。
7. 提交本次任务的所有相关改动，提交后停止。
