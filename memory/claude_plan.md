## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断完成状态，定位第一个未完成任务。
2. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确标注未完成的问题；如果有，将其视为当前任务的一部分或按要求在 `TODO.md` 中补成前置任务。
3. 阅读当前任务在 `TODO.md` 中的完整要求、依赖、验证标准，并按需查阅相关源码与测试，确认最小正确改动范围。
4. 实现当前任务，不绕过规范、不引入临时兼容方案；若遇到会阻塞该任务的真实缺口，先修复阻塞项，或将其作为最小前置任务写回 `TODO.md`。
5. 运行与当前任务直接相关的验证命令，并在必要时补充或修复测试，直到结果满足任务要求。
6. 更新 `TODO.md`：仅当当前任务真正完成时，为任务标题加上 `[DONE]` 并填写/更新完成记录；若被阻塞，则保持未完成并记录新增前置任务与依赖。
7. 仅在阶段级计划、依赖或完成标准发生变化时更新 `PLAN.md`；否则不改。
8. 在关键进展或计划变化后持续更新本文件，记录当前任务、阻塞、已执行验证和剩余收尾动作。
9. 完成后检查工作区改动范围，按要求创建一次 git 提交，然后停止，不进入下一个任务。

## 约束提醒

- 不泄露内部逐词推理；此文件只记录可执行计划、关键判断依据与进度。
- 只处理 `TODO.md` 中的第一个未完成任务。
- 若任务无法按规范完成，必须先在 `TODO.md` 中显式加入最小前置任务，再提交并停止。

## 当前任务定位

- 已确认第一个未完成任务是 `G7-T08：重建 perform / handle / resume / Step_F lowering`。
- 本次执行按 `TODO.md` 顺序继续推进该任务，不跳到 `G7-T08R` 或 `G8-T09`。
- 接下来会先核对最新提交信息与当前工作树，确认是否存在需要一并收口的同任务残留。

## 当前任务执行步骤

1. 阅读 `G7-T08` 指定的关键文件：`crates/scoopc/src/llvm/codegen/expr.rs`、`crates/scoopc/src/llvm/codegen/mir_body.rs`，并补充查阅与 `EffectOutcome`、`EffectCtx`、continuation resume driver、`Step_F` schema 直接相关的 helper 模块。
2. 运行 `cargo check -p scoopc`，以当前首批错误为准确认 `G7-T08` 的实际断点，避免离开当前任务范围做无关修补。
3. 在不恢复任何 deleted TLS/bridge/runtime helper 的前提下，补齐 `perform` / `handle` / `resume` / `Step_F` lowering 缺口；若发现当前任务被新的真实前置缺口阻塞，则把最小前置任务写回 `TODO.md`。
4. 运行与本任务直接相关的验证，包括至少：`cargo check -p scoopc`、必要的定向 grep、以及能证明 lowering contract 闭合的相关测试或检查命令。
5. 若任务完成，更新 `TODO.md` 的 `G7-T08` 标题为 `[DONE]` 并补全完成记录；仅在阶段级计划变化时改 `PLAN.md`。
6. 检查工作区，按任务要求提交一次 git commit，然后停止。

## 当前进度

- 已确认当前工作树属于 `G7-T08` 续做状态，并在此基础上完成收口。
- 已完成的关键修复：
  1. 收口 `perform` / `handle` / generated continuation resume driver / `Step_F` lowering 的活跃实现，并把相关验证切到新的 refactor surface。
  2. 清理本任务过程中暴露的 dead helpers 与 clippy 噪音，使 `cargo clippy -p scoopc --all-targets -- -D warnings` 通过。
  3. 修复 late-lowered/source `TypeStore` 到 codegen 类型层的映射缺口，使 `RuntimeError` 等 foreign type id 能通过当前 LLVM lowering/layout 路径被正确处理。
- 已完成验证：
  1. `cargo check -p scoopc`
  2. `cargo clippy -p scoopc --all-targets -- -D warnings`
  3. `cargo test -p scoopc`
  4. 定向单测：`state_machine_multi_payload_perform_uses_tuple_transport`、`composed_continuation_resume_*`、`default_single_file_ir_helper_lowers_handle_main_without_hir_fallback`、`refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry`、`refactor_llvm_main_wrapper_routes_unhandled_outward_to_exit_code`
  5. `cargo run -p scoop -- build tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop -o /var/folders/0s/mcfxhz813ps4mky0c1sr7rz00000gn/T/opencode/g7_t08_direct_handle_resume.ll --emit-llvm --opt-level 0`
  6. 对 `crates/scoopc/src/llvm/codegen` 与 `crates/scoopc/src/effect_lowered` 执行 deleted bridge/TLS 名字 grep：无命中。
- 剩余收尾：更新 `TODO.md` 的 `G7-T08` 完成记录，检查工作区后提交并停止。

## 本次会话检查点

- 检查点 1：确认最新提交与当前工作树是否都属于 `G7-T08` 续做。
- 检查点 2：定位当前 verifier / lowering 失败点并修复至 `cargo check -p scoopc` 通过。
- 检查点 3：更新 `TODO.md` 完成记录、补写本文件进度、提交本次结果后停止。
