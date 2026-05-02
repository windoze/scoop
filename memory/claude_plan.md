# Claude Plan

## Current Goal
- 按 `TODO.md` 索引定位第一个未完成的详细任务，并只完成这一个任务。

## Execution Plan
1. 读取 `TODO.md`，确认详细任务文件映射与任务顺序。
2. 按顺序读取对应的 `TODO-Px.md`，以标题是否带 `[DONE]` 判断完成状态，定位第一个未完成任务。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题；若存在，将其视为当前任务的一部分或前置条件。
4. 阅读与当前任务直接相关的代码、测试、文档与任务要求，明确验收标准与依赖。
5. 实现任务；如果遇到阻塞当前任务且不能按规范完成的问题，最小化新增前置任务并同步 `TODO-Px.md` 与 `TODO.md`。
6. 运行与该任务直接相关的验证；至少覆盖任务要求中的检查，并在可行时运行 `cargo fmt`、相关测试，以及 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划调整。
8. 在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并补充完成记录；如索引内容受影响，同步更新 `TODO.md`。
9. 按仓库提交风格创建一次 git commit，然后停止，不继续下一个任务。

## Decision Log
- 不会把内部逐词推理写入仓库文件；这里记录可审计的执行计划、关键依据和进度。
- 仅在当前任务被真实阻塞时才新增前置任务，避免为方便执行而拆分任务。

## Progress Log
- 已创建计划文件，下一步开始读取任务索引并定位第一个未完成的详细任务。
- 已读取 `TODO.md` 并确认首个未完成详细任务为 `TODO-P4.md` 中的 `P4-T05R`。
- 已检查最近一次提交 `[P4-T05] Add effect-facts dump CLI and golden baseline`；提交信息未显式记录新的未完成事项，需要通过本次 review 做定向核查。
- 开始执行 P4-T01 ~ P4-T05 定向验证。
- 在 `cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell` 中发现阻塞性失败：`refactor_callable_effect_facts_shell_skips_effect_op_roots` 断言的 pass-view root 数量与当前实际结果不一致，新增可见 root 为 `sample.raiseString::<Unit>`。

## Current Blocker Investigation
1. 阅读失败测试与相关 canonical pass-view / effect-facts builder 代码，判断新增 root 是正确行为还是回归。
2. 若是实现错误，直接修复实现并补测试；若是旧断言失真，则最小化修正测试并确认它继续锁定真正需要的 contract。
3. 重新运行失败测试与其相邻的 P4 定向测试，确认 blocker 消失后再继续剩余验证。

## Blocker Resolution Notes
- 已确认 `sample_source()` 中存在 6 个非 effect-op callable（`exercise`、`pureUnit`、`raiseString`、`raiseInt`、`pingFlag`、`resumeZero`）；失败来自测试把 contract 错误地硬编码为数量 `5`。
- 已将 `refactor_callable_effect_facts_shell_skips_effect_op_roots` 改为比较 `callable_facts` roots 与 canonical pass-view roots 的集合相等，并继续断言 effect op roots 不进入 facts 键空间。

## Final Status
- 已完成 blocker 修复，并重新跑通 P4-T01 ~ P4-T05 的定向测试、CLI/fixture 命令、legacy unsupported 诊断校验，以及 `clippy -D warnings`（含 `--no-default-features` 与默认特性）。
- review 结论：P4 effect-facts stage 已把 canonical MIR snapshot、schema pool、callable/body/block/site facts、`resolved_outward_cases`、`needs_reentry`、`impl_plan` 与 nested handle 分类稳定收口到 `MaterializedEffectFacts`；`dump-effect-facts` CLI 与 fixture 共用同一 helper；P5 可以只消费 canonical MIR snapshot + P4 facts 进入 late-lowering。
- 已更新 `TODO-P4.md`、`TODO.md`，将 `P4-T05R` 标记为 `[DONE]`；`PLAN.md` 无需改动。

## Review Checklist For P4-T05R
1. 核查 `MaterializedEffectFacts`、schema pool、callable/body/block/site facts、`resolved_outward_cases`、`needs_reentry`、`impl_plan` 是否都以 refactor facts 子系统为 authoritative 输出。
2. 核查 `dump-effect-facts` CLI、stable formatter、fixture phase、golden 基线是否共用同一 stage helper，且 legacy 路径为稳定 unsupported 诊断。
3. 核查代码或注释中是否明确冻结 P4 -> P5 handoff：P5 只能消费 canonical MIR snapshot + `MaterializedEffectFacts`，不得回 HIR/typecheck 补语义。
4. 重新执行 P4-T01 ~ P4-T05 要求的定向验证，并补充必要搜索以确认没有遗留的 HIR / legacy 旁路。
5. 若 review 发现直接影响 P5 handoff contract 的缺口，先修复该问题并补测试；否则更新 `TODO-P4.md` / `TODO.md` / 本计划文件并提交。
