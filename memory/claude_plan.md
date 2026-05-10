# Claude Plan

## 说明

按要求先记录可执行计划与关键决策依据。我不会记录逐字内部推理，但会持续更新高层计划、当前步骤、发现的问题和完成状态，便于检查进度。

## 初始执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断首个未完成任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务的一部分或作为必须先补入 `TODO.md` 的前置任务。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证方式和完成记录；必要时查看 `PLAN.md`，仅用于理解阶段性依赖，不做例行更新。
4. 检查工作区当前状态，避免覆盖我未创建的改动；只围绕当前任务涉及的文件开展实现。
5. 在代码库中定位与当前任务直接相关的模块、测试、夹具和文档，确认现有实现边界。
6. 实现当前任务要求的最小正确改动；若遇到阻塞当前任务的真实缺口或规格不匹配，不做绕过，而是在 `TODO.md` 中补入最小前置任务并调整依赖顺序。
7. 运行当前任务要求的验证，以及必要的针对性测试；如有失败，立即修复并重新验证。
8. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
9. 若任务完成：在 `TODO.md` 中将该任务标题标记为 `[DONE]`，补充完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
10. 按仓库约定创建一次提交，提交信息包含对应任务号；然后停止，不继续下一个任务。

## 当前状态

- 状态：已完成任务识别，进入实现前的定界阶段
- 首个未完成任务：`G2-T03`（重建 backend-owned `EffectOutcome` / transport primitive）
- 最近提交：`[G1-T02R] Review explicit hidden ABI skeleton`
- 最近提交结论：提交信息本身未显式声明与 `G2-T03` 直接相关的额外未完成 issue；按 `TODO.md` 继续执行 `G2-T03`
- 工作区状态：当前仅有 `memory/claude_plan.md` 被修改，用于记录本次执行计划与进展
- 当前步骤：实现 `G2-T03` 所需的 backend-owned outcome/transport primitive

## 当前任务的收敛计划

1. 在 `crates/scoopc/src/llvm/codegen/` 下新增一个 neutral helper 模块，集中放置：
   - `EffectOutcome` / `EffectSignal` / `ValueTransport` 的 builder/query helper；
   - `coerce_u64_word(...)`；
   - task transport tuple 的识别与拆分 helper。
2. 将 `MainCodegen` 上当前直接缺失的 outcome/transport 调用点接到该模块提供的方法上，优先覆盖 `effect_lowered/body.rs`、`effect_lowered/value.rs`、`enum_lowering.rs`、`intrinsics/{containers,thread}.rs`。
3. 在 `runtime_symbols.rs` / `runtime_abi.rs` 中补回 thread resume transport 所需的中性 runtime 声明（仅声明，不恢复任何 effect/continuation bridge）。
4. 删除 `effect_lowered/body.rs` 中对 `declare_runtime_effect_set_active_with_trace` 的遗留依赖，确保本任务验证不再出现该旧 bridge 名字。
5. 运行 `cargo fmt`、`cargo check -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings`（若 clippy 受后续任务缺口阻塞，则记录到完成记录中，至少确保本任务范围内无新增 warning）。
6. 若验证表明后续剩余错误已切换到 `TODO.md` 后续任务（如 `G4/G6/G7`），则更新 `TODO.md` 的 `G2-T03` 完成记录并提交。

## 最新进展

- 已新增 `crates/scoopc/src/llvm/codegen/effect_outcome.rs`，集中放入 explicit outcome/transport primitive。
- 已补 `runtime_symbols.rs` / `runtime_abi.rs` 中的 thread resume transport runtime 声明。
- 已从 `effect_lowered/body.rs` 删除 `declare_runtime_effect_set_active_with_trace` 的遗留调用与 helper。
- `cargo fmt` 已通过。
- `cargo check -p scoopc` 复查结果：`alloc_effect_outcome_slot`、`effect_outcome_is_propagating`、`effect_outcome_payload_transport`、`decode_effect_transport_value`、`coerce_u64_word`、`split_task_transport_tuple_value`、thread resume transport runtime declarations 的缺失错误已不再出现；当前剩余错误主要切到 `G4/G6/G7` 及一个本次引入的小型类型不匹配修正项。
- 已修复该类型不匹配，并同步把 `crates/scoopc/src/llvm/tests.rs` 中指向已删除 `codegen/effect/*` 文件的 `include_str!` 改到现有 target-shape 文件，避免 lint/test 目标被陈旧源清单阻断。
- 再次运行 `cargo check -p scoopc`：本任务目标中的缺失项继续保持消失；剩余错误集中在 `emit_ordinary_call_effect_propagation_check` / `ordinary_effect_propagation_enabled` / `known_fun_body_may_outward_effect` / `codegen_mir_*call*` / `codegen_perform_expr` / `emit_raise_runtime_error_variant` 等后续任务缺口。
- 运行 `cargo clippy -p scoopc --all-targets -- -D warnings`：不再出现 deleted `codegen/effect/*` 文件的 `include_str!` 阻塞；当前仍因上述后续结构性缺口失败，因此无法在本任务内拿到完整 clippy 通过结果。
