# Claude Plan

说明：按安全要求，这里记录可执行计划、关键决策与进展摘要，不记录内部推理细节。

## 初始执行计划

1. 读取 `TODO.md`，识别第一个标题未加 `[DONE]` 的任务。
2. 检查最近一次提交信息，确认是否有与该任务直接相关且明确未完成的问题需要并入当前任务或作为前置任务写回 `TODO.md`。
3. 阅读任务条目中的要求、依赖、验证标准与完成记录，并查看相关代码与测试位置。
4. 在不绕过规范、不缩小范围的前提下，完成该任务所需的最小正确修改。
5. 运行任务要求的验证，以及必要的相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`：若任务完成，则在标题前加 `[DONE]` 并补全完成记录；若遇到真实阻塞，则插入最小前置任务并保持当前任务未完成。
7. 仅在阶段计划确实变化时更新 `PLAN.md`。
8. 提交本次全部相关改动，提交信息使用当前任务号。
9. 停止，不继续下一个任务。

## 进展记录

- 已创建计划文件，下一步读取 `TODO.md` 并确认当前任务。
- 已读取 `TODO.md` 与 `TODO-P8.md`，确认第一个未完成任务为 `P8-T02`：删除 legacy effect/continuation lowering 主线、legacy LLVM effect backend，以及所有 code-shape-specific 旧入口。
- 已检查最近一次提交信息：`[P8-T01R] Review selector removal results`，未见直接要求并入当前任务的显式未完成事项。

## 当前任务执行计划（P8-T02）

1. 盘点当前工作区状态，避免误覆盖已有未提交改动。
2. 搜索并阅读 `P8-T02` 指定的 legacy effect/state-machine、legacy LLVM effect backend、`production_lowered_hir` 兼容链路与 code-shape-specific 旧入口。
3. 判断哪些模块可直接删除，哪些必须先抽离中立 helper 后再删 legacy 逻辑。
4. 用最小正确修改删除旧主线代码、收窄仍保留的中立 API，并同步更新模块导出、注释与相关测试。
5. 增加或更新 `P8-T02` 要求的定向测试/守护。
6. 运行相关测试与搜索，再运行 `cargo fmt` 和 `cargo clippy --all-targets -- -D warnings`；若任务范围内需要，补充更直接的命令验证。
7. 更新 `TODO-P8.md` 与 `TODO.md` 的完成状态和完成记录；仅当阶段计划变化时再更新 `PLAN.md`。
8. 提交本次改动并停止。

## 关键发现与调整

- 当前工作区已有一批与 `P8-T02` 直接相关的未提交删除；其中 `llvm/codegen/call/resume.rs`、`effect` 目录里的部分 suspendability/ordinary-callee 规划代码被一并删掉，但 `llvm/codegen` 现行路径仍在消费这些 helper。
- 因此不直接回退整个旧主线，而是改为：
  1. 保持旧 `state_machine_emitter`、旧 bridge 的删除方向；
  2. 恢复并瘦身现行 backend 仍依赖的共享 helper（ordinary callee suspend plan / suspendability 分析 / call resume ABI helper）；
  3. 继续移除 HIR `handle` 旧 state-machine lowering 入口，避免旧 effect backend 重新回流。

## 完成摘要

- 已删除旧 `effect/state_machine` 的 `segments.rs` / `transform.rs`、`effect/step_summary.rs`、`llvm/codegen/effect/state_machine_bridge.rs`、`llvm/codegen/effect/state_machine_emitter.rs`。
- 已把 `crates/scoopc/src/effect/*` 收窄为仅承载当前 backend 仍需的共享 suspendability / ordinary-callee 分析；新增 `llvm/codegen/effect/ordinary_callee.rs` 承接现行 codegen bridge。
- 已删除旧 HIR `handle` state-machine lowering 入口与一批已无调用的 legacy runtime ABI/helper；`Continuation.resume` replay shim、旧 effect dispatch subtype helper、旧 continuation/handler-stack dead ABI 均已清理。
- 已把 `production_lowered_hir` 族命名改为 `materialized_lowered_hir`，把 `legacy_eager_hir` 改为 `direct_lowered_hir`，并把 `begin/finish_legacy_effect_boundary` 改为中立的 effect boundary helper 命名。
- 已新增定向守护测试，确认 legacy backend marker 只剩负向测试文本。

## 已执行验证

- `cargo check -p scoopc --features llvm`
- `cargo fmt`
- `cargo test -p scoopc legacy_effect_backend_removed`
- `cargo test -p scoopc single_effect_lowering_path`
- `cargo clippy --all-targets -- -D warnings`
- `rg -n "state_machine_bridge|state_machine_emitter|UnifiedHandleLoweringContract|begin_legacy_effect_boundary|finish_legacy_effect_boundary|production_lowered_hir|legacy_eager_hir|single perform|tail-resume|statement-only|linear body" crates/scoopc/src crates/scoop/src tools --glob '!target/**'`
- 搜索结果：仅剩 `crates/scoopc/src/llvm/tests.rs` 新增负向守护测试中的字面量命中，不再有主实现/主 codegen/主 lowering 命中。
