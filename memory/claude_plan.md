# Claude Plan

## Scope

本文件记录本次调用的执行计划、关键进展和验证结果。为避免记录不可公开的内部推理，这里只写可审计的简明依据、决策和步骤。

## Initial Plan

1. 读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务，并确认其依赖、要求和验证项。
2. 查看最新提交信息；只有当最新提交明确提到与当前任务直接相关的未完成问题时，才把它纳入当前任务或作为前置项记录到 `TODO.md`。
3. 针对第一个未完成任务读取必要的相关代码、测试、规范和上下文，避免无关的历史问题扫查。
4. 若任务可直接完成，做最小且完整的实现，并添加或更新最相关的测试/fixture。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，然后运行相关测试；如代码变更需要完整验证，再运行 `cargo test --all --all-targets` 和 `python3 tools/run_fixtures.py`，两者使用足够长的超时。
6. 如发现未被计划覆盖的失败测试/fixture，先修复；若不能在当前任务内合理修复，则在 `TODO.md` 中插入最小前置任务并停止。
7. 完成后将当前任务标题标记为 `[DONE]`，补全完成记录；仅当阶段计划实际变化时才更新 `PLAN.md`。
8. 检查工作区状态和差异，提交本次任务相关全部变更，提交信息使用任务编号和简洁说明。
9. 提交后停止，不进入下一个任务。

## Progress

- 已创建初始执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已确认 `TODO.md` 指向 `TODO-3.md`，第一个未完成任务是 `T3-04A0`。
- 最新提交 `550776e0 [T3-04A] Schedule source-body intrinsic metadata prerequisite` 与当前任务直接相关，说明 `T3-04A0` 是为解除 `T3-04A` fixture 阻塞而插入的前置任务；本次将直接执行该任务。
- 复现代表 fixture：`intrinsic_sysroot_overlay_scalar_method_basic`、`fun_call_add_basic`、`cross_file_generic_top_level_val_basic` 当前通过；`cross_file_ctor_named_default_basic` 失败，输出 `:`/`:`，说明 class ctor source-body f-string 的 builtin `ToString.toString` metadata 不足，落到了接口默认实现。
- 已开始实现：HIR facts 对 legacy scalar `@Intrinsic` 无参标注发布 named entry；随后发现单纯在 LLVM handoff 按 receiver type 选择 scalar toString runtime entry 会绕开 sysroot overlay body，因此改为上游发布 concrete source call 身份。
- 显式重建 `scoop`/`scoopc` 后，`cross_file_ctor_named_default_basic` 与相关 scalar/toString/source-body 代表 fixture 已通过；后续进入 lint 与全量验证。
- 全量 fixture 暴露 overlay `toString` 与 generic `println<T: ToString>` 相关失败；已调整方案，改为 HIR f-string 对 builtin scalar/string 直接发布 concrete `*.toString` source call，并保留 overlay body 语义。另修复无 `else` 的 `if` 在终止 then 分支场景下不再向非 Unit 结果槽写 Unit；effect-lowered `ToString.toString` 仅对 builtin receiver 使用 runtime/string intrinsic 快路，非 builtin receiver 回到已发布 plain callable。相关失败 fixture 已逐个通过。
- 最终验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`（1664 checks）。
- 已将 `TODO-3.md` 的 `T3-04A0` 标记为 `[DONE]` 并补全完成记录；`TODO.md` 当前活跃任务已推进到下一项 `T3-04A`。下一步检查 diff/status 并提交。
