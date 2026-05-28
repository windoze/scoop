本文件记录本次调用的可审阅执行计划与进度。为避免暴露私有推理，这里记录任务判断、执行步骤、验证策略和关键进度更新。

## 初始执行计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判断第一个未完成任务，不进行无关历史问题扫查。
2. 检查最近提交是否明确提到与该任务直接相关的未完成问题；只在其阻塞当前任务时纳入范围。
3. 阅读当前任务涉及的代码、测试、规格或 fixture，确认任务要求与依赖。
4. 若发现当前任务被缺失语言能力、实现边界或未排期失败阻塞，则按要求更新 `TODO.md`，必要时更新 `PLAN.md`，提交后停止。
5. 若可直接完成，则实施最小正确改动，避免绕过规格或 fixture-only hack。
6. 按顺序验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关测试、必要时完整 `cargo test --all --all-targets` 和 `python3 tools/run_fixtures.py`。
7. 更新 `TODO.md`：在当前任务标题加 `[DONE]`，填写完成记录与验证结果；仅当阶段计划变化时更新 `PLAN.md`。
8. 检查 git 状态和 diff，提交本次任务相关改动，然后停止，不处理下一项任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 以确定第一个未完成任务。
- 已确认第一个未完成任务为 `P5-T01R：Review Phase A-C resolution`。最近提交中 `a3924ee5` 是被 review 的 P5-T01 实现，最新 `a3de4718 Update plan` 未显示明确未完成问题；下一步按 review 要求检查候选收集、visibility、applicability、no-applicable diagnostics 和相关 fixture。
- Review 发现 P5-T01 仍有具体阻塞项：部分 typecheck 路径没有消费 resolver 写入的 `ResolvedCall.candidates`，member/inherited/extension 层级仍可能不符合 member-before-extension，跨文件签名 vararg 元数据缺失，部分 no-applicable fixture 未锁定候选签名与原因。由于这些是 P5-T01R 通过前必须修复的 Phase A-C 前置条件，本次改为在 `TODO.md` / `TODO-5.md` 插入最小前置修复任务 `P5-T01a`，并保持 `P5-T01R` 未完成。
- 已完成 `TODO.md` / `TODO-5.md` 的前置任务插入和依赖改写；本次只修改任务文档与本进度文件，未修改编译代码，因此跳过 `cargo fmt`、clippy、Rust tests 和 fixture suite，仅执行 `git diff --check` 检查 patch whitespace。
