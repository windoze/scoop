# 当前任务执行计划

## 约束说明

- 不记录私有内部推理；本文件记录可审查的执行计划、决策点和进度。
- `TODO.md` 是任务顺序和完成状态的唯一权威来源。
- 本次只处理第一个标题未带 `[DONE]` 的任务，完成后提交并停止。

## 初始计划

1. 阅读 `TODO.md`，识别第一个未完成任务及其验证要求。
2. 检查最近提交信息，确认是否有与该任务直接相关的未完成事项。
3. 根据任务内容读取必要的代码、规格和测试上下文。
4. 如发现阻塞当前任务的缺失功能或不符合规格行为，优先修复；若无法在本次正确完成，则在 `TODO.md` 插入最小前置任务并提交后停止。
5. 实现当前任务要求，避免绕过规格或夹带无关修改。
6. 按要求运行格式化、lint、相关测试；需要时运行完整测试和 fixture 套件。
7. 更新 `TODO.md`：给完成任务标题加 `[DONE]`，填写完成记录和验证结果。
8. 仅在阶段计划发生真实变化时更新 `PLAN.md`。
9. 检查 `git status` 和 diff，提交本次任务相关变更。

## 进度日志

- 已创建本计划文件，下一步读取 `TODO.md` 确认当前任务。
- 已读取 `TODO.md` / `TODO-3.md` 与最近提交；本次唯一执行任务为 `T3-04I`。
- 当前任务范围：删除 reflection source-span bridge、class ctor result/span/arg-count/readable-path fallback、generic/value-box 文本恢复、MIR backend fact ABI/source-signature 合成、effect/LIR verifier 静默降级，并补齐 dependency gate。
- 下一步：按任务列出的实际 helper/模式搜索代码，先定位所有仍在生产路径中的残余点，再分小 patch 修复。
- 勘察结果：P6 reflection 仍从 `source_path+span` 查询，class ctor 仍有 result/span/arg-count/readable-path fallback，direct call 仍扫 `source_signatures.keys()`，LIR value-box 仍拼 FQN，MIR backend facts 仍现场合成 source signature/ABI，effect/LIR verifier 与 dependency gate 覆盖不足。
- 编辑策略：先删除 P6 可直接移除的 fallback；再补 MIR/LIR reflection owner+`SiteId` 发布；最后收紧 verifier/gate 并跑验证。
- 已完成主体修改：P6 不再从 reflection source-span bridge 获取类型实参；class ctor readable-path/span-suffix/arg-count fallback 已删除；HIR direct-call root scan 已删除；MIR reflection rvalue 携带 `SiteId` 并发布 LIR reflection facts；effect import 缺 stable key 改 fail-fast；MIR backend 普通 source signature 不再本地 `AbiMangler` 合成 ABI。
- 已通过：`cargo check --all-targets`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。下一步运行完整测试与 fixture。
- 修复验证中暴露的 bodyless source signature target 发布与 class ctor verifier 过严问题后，已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`。
- 下一步：运行 `python3 tools/spec_fixtures.py check` 和 `python3 tools/run_fixtures.py`。
- 已完成最终验证：`python3 tools/spec_fixtures.py check` 与 `python3 tools/run_fixtures.py`（1664 checks）通过。
- 已更新 `TODO-3.md` 将 `T3-04I` 标记为 `[DONE]`，并更新 `TODO.md` 当前活跃任务为 `T3-04R`。
