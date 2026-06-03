# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不包含私密推理链。

## 初始计划

1. 读取 `TODO.md`，按文档顺序识别第一个标题未以 `[DONE]` 前缀标记的任务。
2. 检查最近提交信息；仅当其明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 阅读当前任务所需的相关代码、规格、测试和完成要求，避免做开放式历史问题扫描。
4. 如当前任务可直接完成，则实施最小正确改动，并补充或调整必要测试/fixture。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，随后运行相关测试；如代码改动影响全局行为，再运行完整 Rust 测试和 fixture 套件。
6. 若发现未安排且阻塞当前任务的测试/fixture 失败或规格不匹配，优先修复；无法在当前任务内完成时，在 `TODO.md` 添加最小前置任务并停止。
7. 完成后将当前任务标题加上 `[DONE]`，更新其完成记录和验证记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务相关全部改动，并停止，不继续下一个任务。

## 进度

- 已建立初始执行计划。
- 已读取 `TODO.md` / `TODO-3.md`：首个未完成任务为 `T3-04R：Review T3-04`。
- 已检查最新提交：`f5a8ed06 [T3-04H] Close generic ctor value-box fallbacks`，与当前 review 任务直接相关，作为审查基线。

## T3-04R 执行计划

1. 定向审查 T3-04/T3-04A..H 收口范围内的生产路径和守卫：P6 source side table、FQN/string/generic fallback、class ctor/value-box/dispatch/intrinsic fallback、ABI/source-signature 合成、verifier fail-fast 和 dependency gate 覆盖。
2. 运行现有结构守卫，确认 gate 未发现残余违规路径。
3. 如审查发现阻塞缺口，按最小前置任务更新 `TODO-3.md` 并停止；不把 `T3-04R` 标记完成。
4. 如未发现阻塞缺口，按要求运行验证（至少 `python3 tools/run_fixtures.py`；在完整 fixture 前先执行格式/ lint/结构守卫），然后更新 `TODO-3.md` 和根 `TODO.md` 的状态。
5. 检查差异并提交本次 review 结果，然后停止。

## T3-04R 审查结果

- 已完成定向审查，确认 `T3-04H` 后仍存在阻塞 `T3-04R` 完成的残余缺口。
- 主要缺口：reflection metadata 仍走 `source_path + span`；class ctor 仍有 result/span/arg-count/readable-path fallback；direct-call/value-box 仍有 FQN/root 文本恢复；MIR backend facts 仍有 ABI/source-signature 合成；effect/LIR verifier 仍有静默降级；dependency gate 未覆盖这些实际 helper。
- 已在 `TODO-3.md` 中新增前置任务 `T3-04I`，并将 `T3-04R` 依赖改为 `T3-04I`；根 `TODO.md` 当前活跃任务同步更新为 `T3-04I`。
- 因当前 invocation 的任务是 review 且发现阻塞前置缺口，本次不继续实现 `T3-04I`，将提交任务清单与计划更新后停止。
