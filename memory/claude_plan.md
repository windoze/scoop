# 执行计划

## 当前状态

- 已读取 `TODO.md` / `TODO-3.md`，首个未完成任务是 `T3-04F`。
- 最新提交 `ca8af15a [T3-04R] Schedule sixth review follow-up` 直接对应本任务，应作为当前任务背景处理。
- 工作区已有未跟踪 `FACT_REFACTOR.md`，当前不属于本任务，除非后续确认它与 `T3-04F` 直接相关，否则不修改。
- 勘查确认仍存在：P6 `current_call_site`/`source_call_site_id` bridge、LIR ctor/reflection facts 从 HIR source-site helper 发布、LIR declaration/bodyless ABI 与 source-signature 合成、layout root-only verifier、MIR/P6 FQN/string fallback 以及 dependency gate 漏洞。
- 当前编辑策略：先把 ctor/reflection call-site facts 收口到 LIR owner+`SiteId`，删除 P6 path/span 查询入口；随后收紧 ABI/source-signature 发布、layout/effect verifier 与 gate。
- 已实现主体改造并验证：`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check` 通过。
- 阻塞：完整 `python3 tools/run_fixtures.py` 仍有与本任务直接相关的剩余失败（HIR/source-payload ctor/reflection/atomic、MIR golden、少量 runtime/build residuals）。已在 `TODO-3.md` 插入前置任务 `T3-04F0`，`T3-04F` 保持未完成。
- 本文件用于记录可公开的执行计划、关键进展和验证结果。

## 步骤

1. 定位 `T3-04F` 指定的残余路径：P6 source-span bridge、LIR ctor/reflection HIR source-site 扫描、ABI/source-signature synthesis、layout/effect verifier root-only 校验、FQN/string/唯一候选 fallback 与 dependency gate 漏洞。
2. 阅读相关实现和现有测试，确认每类残余是否仍存在，以及最小正确修复边界。
3. 实现 fact-only / fail-fast 收口：删除或替换残余 source/path/span、FQN/string、root-only、合成 ABI/source-signature 和静默跳过路径。
4. 补齐 verifier / dependency gate / 单测或 fixture，锁定新增契约。
5. 运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`，通过后运行完整 Rust 测试、build、dependency gate、spec fixtures check 和完整 fixture suite。
6. 更新 `TODO-3.md` 与 `TODO.md` 状态和完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查 git status/diff/log，提交本次任务相关变更，然后停止。

## 注意事项

- 每次只完成 `TODO.md` 中的第一个未完成任务。
- 不使用 workaround；遇到阻塞性 spec 或实现缺口时，先修复或插入前置任务并停止。
- 不修改无关用户变更，不回退他人改动。
