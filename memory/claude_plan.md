## 执行计划

1. 读取 `TODO.md`，确认索引中的任务顺序与对应详细文件引用。
2. 按顺序检查相关 `TODO-Px.md`，以标题是否带有 `[DONE]` 为准，定位第一个未完成的详细任务。
3. 查看最近提交记录，确认是否存在与该任务直接相关且未收尾的问题；如存在且构成前置依赖，先按要求在详细 TODO 中补充前置任务并同步索引。
4. 阅读当前任务的详细要求、约束、依赖和验收标准，结合代码现状实现该任务。
5. 运行与该任务相关的测试、格式化、静态检查；若出现问题，立即修复，直到通过或确认存在必须新增的前置任务。
6. 更新 `memory/claude_plan.md` 记录关键进展。
7. 在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并补充完成记录；如索引受影响，同步更新 `TODO.md`。
8. 若阶段计划未变化，不改 `PLAN.md`；仅在阶段依赖或完成标准变化时更新。
9. 按要求创建一次 git 提交，只完成当前这个任务后停止。

## 当前状态

- 已读取 `TODO.md`，确认首个未完成详细任务为 `TODO-P6.md` 中的 `P6-T02a`。
- 已检查最新提交：`[P6-T02a] Track authoritative resume-interface blocker`，该提交把当前问题登记为前置任务，没有留下额外未收尾代码。
- 已完成 LLVM ABI materializer 收口：
  1. 删除了按 `(step_schema, effect_family)` 在 P6 现场补造 `ResumeInterfaceId` 的逻辑；
  2. callable/continuation/resume-interface layout 现严格消费 `LateLoweredProgram.resume_interfaces()`、`LateLoweredCallable.resume_interfaces()`、`LateLoweredContinuationObject.implemented_interfaces()`；
  3. 对缺失 interface、重复发布、return-step 不匹配、method contract 漂移等情况都会直接 fail fast。
- 在验证阶段发现一个真实 blocker：ABI-visibility late-lowered program 仍沿用 authoritative reachable-body 的后处理裁剪，把 unreachable helper 需要发布的 resume interface/method 壳层删掉了，导致两个 refactor build fixture 失败。
- 已继续修复该 blocker：为 ABI-visibility handoff 新增“保留 published resume shells”的 late-opt 模式，只影响 ABI shell 可见性 program，不改变 authoritative reachable-body program 的原有裁剪行为。
- 已补充/更新定向单测，覆盖：
  1. callable/object authoritative interface 顺序保真；
  2. authoritative method 顺序保真；
  3. 缺失 authoritative interface 时 fail fast；
  4. `Unit` ABI 与完整 method 集场景。
- 已完成验证：
  1. `cargo test -p scoopc refactor_llvm_step_layout`
  2. `cargo test -p scoopc refactor_llvm_continuation_layout`
  3. `cargo test -p scoopc refactor_llvm_unit_abi`
  4. `cargo test -p scoop refactor_build_publishes_request_source_abi_shells_for_unreachable_effectful_helpers`
  5. `cargo test -p scoop refactor_build_rejects_reachable_self_contained_legacy_effect_body_lowering`
  6. `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  7. `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_dynamic_invoke_unit_payload.scoop`
  8. `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_continuation_interface_full_methods.scoop`
  9. `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 下一步：更新 `TODO-P6.md` / `TODO.md` 的完成标记与完成记录，检查工作区后创建本次任务提交。
