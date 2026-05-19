# Claude Execution Plan

本文件记录本次调用的可审查执行计划与进度更新；不包含私密逐步推理。

## 当前计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 开头的任务。
2. 查看最近提交信息，仅在其明确提到与当前任务直接相关的未完成问题时，将该问题纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 阅读当前任务要求、依赖、验证条件和相关代码/测试，避免进行无关历史问题扫查。
4. 如当前任务存在必须先修复的规格不匹配或缺失能力，更新 `TODO.md` 添加最小前置任务并提交后停止。
5. 否则实现当前任务，优先做最小且完整的正确修改。
6. 运行当前任务要求的验证命令及必要的相关测试；若失败，定位并修复属于当前任务范围的问题。
7. 更新 `TODO.md`：给完成的任务标题添加 `[DONE]` 前缀，并补充完成记录；仅当阶段计划变化时才更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务相关的所有未提交改动。
9. 提交后停止，不继续处理下一项任务。

## 进度

- 已创建初始执行计划，下一步读取 `TODO.md` 识别首个未完成任务。
- 已读取 `TODO.md`，首个未完成任务为 `P7-B3.4：B-29 GC intrinsic contract`。
- 下一步检查最近提交与工作区状态，仅纳入与 B-29 GC intrinsic contract 直接相关的未完成问题。
- 最近提交为 `[P7-B3.3] Retire atomic UMB rows`，未发现明确指向 B-29 的未完成项；工作区除本计划文件外无其他改动。
- 下一步读取 B-29 audit/strategy/fixture/codegen 信息，确定 93 个 active rows 的具体退场位置。
- 已定位 B-29 的 93 个 active rows：主要在 `gc.rs`、`mir_body/member.rs`、`effect_lowered/value.rs`、`intrinsics/named.rs`、`main/frame.rs`、`main/gc_locals.rs` 与 `effect_outcome.rs`。
- 已确认 typecheck 已有 GC intrinsic 用户面 gate；本次将补 MIR production/materialized GC contract，并把对应 LLVM fallback 改为内部 invariant/expect helper。
- 已完成代码迁移并通过 `cargo check -p scoopc`；B-29 `UnsupportedMainBody` rows 已从 active inventory 移入 retired ledger，`umb-audit diff` 显示同步，`umb-audit list --bucket B-29` 显示 0 entries。
- 已激活 `tests/fixtures/umb_fix/B-29-gc-intrinsics/`，正/负 fixture 均通过；下一步运行 audit、policy、runtime 和 clippy 验证。
- 已完成 required/补充验证：audit、failure policy、MIR materialize、B-29 fixtures、`scoop_runtime`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已更新 `TODO.md`：`P7-B3.4` 标记 `[DONE]`，完成记录已写入；当前统计为 active=320、retired=964。
- 下一步检查 git diff/status，确认无无关改动后提交本任务。
