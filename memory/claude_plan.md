# 当前执行计划

## 范围

- 按 `TODO.md` 的顺序只处理第一个未标记 `[DONE]` 的任务。
- 不做开放式历史问题扫描；只处理当前任务直接需要的问题，或验证过程中暴露且未被明确排期的失败。
- 若遇到阻塞性缺口，不绕过实现；在 `TODO.md` 中加入最小必要前置任务并停止。

## 步骤

1. 阅读 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录其要求、依赖与验证方式。
2. 查看最近提交信息，确认是否有与该任务直接相关的未完成事项需要纳入当前范围。
3. 检查相关代码、测试、fixture 与文档，明确最小正确改动面。
4. 实现当前任务；如发现必须先修复的语言/运行时/测试缺口，则更新 `TODO.md` 排入前置任务并停止。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，然后运行相关测试；如代码变更影响全局行为，再运行完整测试与 fixture 套件。
6. 修复所有未明确排期的失败，或将其作为最小必要任务加入 `TODO.md`。
7. 完成后在 `TODO.md` 中给任务标题加 `[DONE]`，更新完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查 git diff/status，提交本次任务相关所有变更，并停止，不进入下一项任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P5-T01a：修复 Phase A-C review blockers`。
- 最新提交 `c20b141f [P5-T01R] Schedule Phase A-C review blockers` 与当前任务直接相关，按要求纳入当前范围。
- 已完成主要实现改动：普通调用消费 `ResolvedCall.candidates`，extension late lookup 进入统一 applicability，constructor 使用统一 default/vararg mapper，跨文件签名保留 `vararg`，member-before-extension 顺序覆盖 inherited member，where-bound 方法先收集全部候选再过滤。
- 已完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、targeted Phase A-C fixtures、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc && python3 tools/run_fixtures.py` 均通过。
- 已将 `TODO.md` 与 `TODO-5.md` 中 `P5-T01a` 标记为 `[DONE]` 并填写完成记录；下一步检查 git diff/status 并提交本任务变更。
