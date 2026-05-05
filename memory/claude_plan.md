# 当前执行计划

## 推理摘要

- 目标是严格按 `TODO.md` 索引和对应 `TODO-Px.md` 详情文件找到第一个未完成任务。
- 完成且只完成一个详细任务；任务完成必须在详细文件标题中加 `[DONE]`，并同步 `TODO.md` 索引。
- 若当前任务被真实实现缺口阻塞，不做绕路实现；改为在对应详细 TODO 中插入最小必要前置任务，同步索引，提交后停止。
- 不把 `PLAN.md` 当作日常日志；只有阶段级计划或依赖结构变化时才更新。
- 最终需要提交本次变更；不继续处理下一个任务。

## 步骤计划

1. 读取 `TODO.md`，确认索引顺序和引用的详细任务文件。
2. 按索引顺序读取对应 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 读取该任务要求、约束、依赖和验证命令；检查最新提交是否提到与该任务直接相关的未完成问题。
4. 检查当前工作树状态，区分已有改动和本次将修改的文件，避免回退用户改动。
5. 基于任务要求检查相关代码、测试和夹具，确认最小正确实现路径。
6. 实现当前任务；若发现阻塞性实现缺口，改为添加最小前置任务并同步索引后停止。
7. 运行相关测试；必要时修复失败并重新验证。
8. 更新对应 `TODO-Px.md` 的任务标题为 `[DONE]` 并填写完成记录；同步 `TODO.md` 中同一任务的 `[DONE]` 状态。
9. 如执行计划发生关键变化或关键步骤完成，更新本文件。
10. 运行最终必要验证，检查工作树差异。
11. 按要求提交所有相关未提交文件，并停止，不进入下一任务。

## 当前任务

- 已定位第一个未完成详细任务：`TODO-P7.md` 的 `P7-T02Y`。
- 任务目标：修复 `tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop` 在默认 refactor 路径下 nested escaped-continuation replay 穿过 arm-local handle 后未继续执行 tail 的阻塞。
- 初始验证目标：复现 `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop` 和 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop` 的失败。
- 实现方向：从 late-lowered / LLVM continuation replay 与 handle-in-arm continuation composition 入手，保留 `P7-T02X` 的 cross-call provenance 与 resume-boundary composition 语义，不引入 fixture 特判或 legacy fallback。

## 关键发现

- 失败已复现：默认运行只输出到 `boom_arm`。
- 直接构建后用 lldb 观察到崩溃发生在 `__scoop_refactor_surface_resume_owner_dispatch__start__k5` 内的 GC write barrier。
- 根因是首次 `k.resume(7)` 的 resume owner trampoline 进入外层 `Boom` arm 时，需要读取 `cell` 参数执行 `cell.saved = Some(k)`，但 frame lifting 没有把该参数纳入 continuation frame；resume entry 恢复后 `cell` 为空，导致写屏障访问无效对象。
- 修复方向调整为：在 `effect_lowered/frame.rs` 的 liveness 中加入 handle boundary routing 的动态控制流边，确保 boundary outward 被 handle arm 消费时，该 arm 后续所需 locals 会被提升进 frame。
- 已补 frame-liveness 后程序继续运行，但语义仍错误：首次 `k.resume(7)` 在 owner trampoline 内被外层 `Boom` handle 直接消费并继续执行 inner arm tail，导致 `after_start` 输出 `119` 而不是先返回 `18`。
- 追加修复方向：在 surface resume owner trampoline 中，`handle_boundary_action` 只能让当前 surface route 发布的 handle site 消费/pending；其它 handle 对该 outward case 必须继续 outward，交还给原始 resume boundary 组合 continuation。

## 完成记录

- 已实现 `P7-T02Y`：frame liveness 纳入 handle routing 动态边；surface resume owner trampoline 限制非当前 surface route 的 handle consumption。
- 目标程序已输出 golden 序列，并且 fixture harness 通过。
- 已更新 `TODO-P7.md` 标题为 `[DONE] P7-T02Y` 并填写完成记录；已同步 `TODO.md` 索引。
- 已通过验证：`cargo fmt --all`、`cargo test -p scoopc --lib refactor_frame_lifting_captures_locals_used_by_routed_handle_arm`、`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`、`cargo test -p scoopc --lib effect_lowered`、`cargo test -p scoopc --lib llvm::codegen::effect_refactor`、`cargo clippy --all-targets -- -D warnings`。
- 下一步：检查 diff/status 后提交本任务变更并停止。
