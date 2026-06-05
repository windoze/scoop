# Claude 执行计划

## 范围

- 以 `TODO.md` 作为权威任务列表和完成状态来源。
- 只选择第一个标题未带 `[DONE]` 的任务并完成它，然后停止。
- 除非阶段级计划或依赖关系实际变化，否则不更新 `PLAN.md`。

## 通用步骤

1. 读取 `TODO.md` 并识别第一个未完成任务。
2. 检查当前工作区状态和最新提交，确认是否有与该任务直接相关的未完成事项。
3. 阅读当前任务涉及的最小必要代码、测试、fixture 和文档。
4. 按任务要求实现，不通过缩小范围、FQN 反查、shim 或 fixture-only hack 绕过问题。
5. 添加或更新必要的测试/fixture。
6. 先运行 `cargo fmt`。
7. 再运行 `cargo clippy --all-targets -- -D warnings`。
8. 按任务要求运行相关测试、完整 Rust 测试、构建和 fixture 基线。
9. 若发现未排期失败，修复它或在 `TODO.md` 中加入最小必要前置任务。
10. 完成后在 `TODO.md` 中给任务标题加 `[DONE]` 并更新完成记录。
11. 检查差异并提交本轮任务变更。

## 当前任务

- 已选择第一个未完成任务：`TC-04-FIX2`。
- 目标：清除 carrier/dispatch 生产路径中仍以 callable FQN/root 字符串做 live target/layout/facts 选择的残留，改为 `LirCallableRef`、stable callable hash 或 body-version/contract 查询。
- 最新提交 `ee793352 [TC-04-R] Schedule carrier FQN lookup fix` 直接对应当前任务。

## 进度

- 已确认初始验收 grep 命中集中在 carrier target map、dynamic invoke candidate target、carrier shell 发布、class vtable/itable global、static interface dispatch 和 fallback registry。
- 已将 carrier target registry 从 `(CallableCarrierKind, String)` 改为 `CallableCarrierTargetKey { kind, LirCallableHash }`。
- 已将 dynamic invoke candidate target、carrier layout 查询、entry symbol registry、plain fallback registry、vtable/itable target declaration 和 static interface dispatch 改为消费 LIR callable handle/hash。
- 已将 physical layout 消费从 `impl_member_fqn` / `method_impl_fqns` 切到 `impl_member_target` / `method_impl_targets`；FQN 仅保留为符号名、source-signature 文本、诊断或 nominal/global layout key。
- 已为 callable layout 保存 stable `LirCallableHash`，修复 external hash 与 callable layout 的匹配路径。
- 验收 grep 已通过：carrier target map、physical FQN target field、旧 FQN lookup helper 三组生产路径 grep 均无命中。

## 验证记录

- `cargo fmt` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- `cargo test --all --all-targets` 通过。
- `cargo build -p scoop -p scoopc` 通过。
- `python3 tools/dependency_gate.py` 通过。
- `python3 tools/spec_fixtures.py check` 通过。
- `python3 tools/run_fixtures.py` 通过。

## 下一步

- `TODO.md` 已更新 `TC-04-FIX2` 完成记录。
- 下一步检查最终差异并提交本轮任务变更。
