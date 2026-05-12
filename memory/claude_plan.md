# Claude Plan

## 当前执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否有与该任务直接相关且未完成的问题需要一并处理，或作为新的前置任务写回 `TODO.md`。
3. 阅读该任务涉及的实现与测试文件，确认需求、依赖、现状与可能阻塞项。
4. 如无阻塞，直接完成该任务的最小正确实现；如有真实阻塞，按要求把最小前置任务写入 `TODO.md`，并停止在该前置变更处。
5. 运行该任务要求的验证与必要的相关测试，修复出现的问题，直到结果稳定。
6. 更新 `memory/claude_plan.md` 记录关键进度；完成后把任务在 `TODO.md` 中标记为 `[DONE]`，补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 按仓库约定创建一次提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md` 并确认当前第一个未完成任务已前移为 `G8-T13：按 PIPELINE_GAPS 收口当前剩余 fixture failure`；`G8-T12` 已在 `TODO.md` 中标记为 `[DONE]`。
- 已检查最新提交信息；最近提交标题为 `Update plan`，未显式声明与 `G8-T13` 直接相关的未完成问题，因此继续按 `TODO.md` 当前顺序执行。
- 已确认当前工作树存在未提交修改，且集中在 `G8-T13` 涉及的 codegen/effect-lowered 路径；后续会把这些修改一并纳入当前任务分析，不回退、不忽略。
- 已复现 `G8-T13` 当前列出的 4 个 fixture：
  - `handle_arm_explicit_type_args_basic.scoop` 仍在编译期失败，错误为 `refactor LLVM ABI query 缺少 source type 387 的 ABI value lowering contract`。
  - `effect_typed_plain_adapter_aggregate_return_basic.scoop` 运行时只输出 `41`、`42` 后退出码为 `1`。
  - `effect_typed_plain_adapter_multiple_effect_rows_basic.scoop` 运行时退出码为 `1`。
  - `stdlib_smoke_test_and_preconditions.scoop` 已从“运行到 all_passed 后挂起”前移为编译期失败：当前未提交 resume-boundary wrapper projection 改动引入了 owner-step -> wrapper-step projection 歧义。
- 已通过 dump 确认：`handle_arm` 的 `Query.ask<Int>` 在 `effect_facts` / `effect_lowered` 中仍被建成 `Query.ask<T>`、`resume_tuple_ty=T`，说明问题起点早于 LLVM codegen；需要把 op type args/typed contract 沿 HIR -> MIR -> effect_facts 继续传下去，或让 effect-facts 的 callable case seed 能以这些 concrete args 取代 generic op case。
- 已确认 `stdlib_smoke` 的新失败直接来自 `crates/scoopc/src/effect_lowered/ir.rs` 当前 resume-boundary wrapper projection 发布逻辑：同 owner/schema 但不同 boundary local 的 projection 被当成不同 shape，需要按 publication/source-shape 去重，而不是把 boundary 局部 local identity 误当成语义差异。
- 已完成代码修复并重新验证：4 个 `G8-T13` 明确列出的 fixture 现已全部在 `scoop run` 与 `scoop test --fixtures ...` 层恢复通过；相关 targeted tests 也已通过。
- 已重跑 authoritative full scan：默认环境 `1225 total / 1164 pass / 61 fail / 0 timeout`；full-GC 环境 `1225 total / 1162 pass / 63 fail / 0 timeout`。这表明当前 4 个已知 blocker 已收口，但 full inventory 仍有大量后续剩余 failure，必须在 `TODO.md` 中新增下一任务显式承接，不能再把它们继续塞在 `G8-T13` 下。

## 当前细化计划（G8-T13）

1. 读取 `G8-T13` 条目全文与 `PIPELINE_GAPS.md` 中指定条目，确认 4 个已知失败分别对应的 gap owner。
2. 审查当前工作树中已存在的未提交修改，判断它们是否已经部分覆盖 `handle_arm_explicit_type_args_basic`、plain adapter 挂起或 stdlib smoke 退出挂起中的某一簇问题。
3. 先复现当前 4 个已知失败 fixture，确认哪些仍然失败、失败形态是否已变化，以及是否需要先更新 `PIPELINE_GAPS.md` 的 gap 描述。
4. 以 gap 为单位完成修复；若发现当前任务无法绕过的真实新前置缺口，则把最小前置任务写入 `TODO.md` 并停止。
5. 当前 4 个 fixture 全绿后，按任务要求重跑 default/full-GC 双环境 fixture scan 与 `cargo test --all`，并同步更新 inventory/gap 文档。
6. 完成后把 `G8-T13` 标记为 `[DONE]`，更新本计划文件，并按仓库约定提交一次包含当前任务全部未提交文件的提交。

## 本轮执行更新

1. 重新读取 `TODO.md`，确认当前第一个未完成任务是 `G8-T13`，并以 `TODO.md` 为唯一准绳。
2. 查看最近一次提交标题，判断是否存在与 `G8-T13` 直接相关且被明确标为未完成的问题；若有，则纳入本轮任务或写成前置任务。
3. 读取 `G8-T13` 及其相关 gap/失败说明、实现代码和测试入口，缩小到本轮必须解决的失败集合。
4. 复现当前失败；如果出现阻塞当前任务的真实缺口，则先把最小前置任务写回 `TODO.md` 并停止；否则直接修复。
5. 运行 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings` 等必要验证。
6. 完成后更新 `TODO.md` 的 `[DONE]` 状态与完成记录，更新本文件进度，按仓库约定提交一次并停止。
