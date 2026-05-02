## 当前执行计划

1. 读取 `TODO.md`，确认它仅作为任务索引使用。
2. 按 `TODO.md` 引用顺序检查对应的 `TODO-Px.md` 详细任务文件。
3. 找到第一个标题未标记 `[DONE]` 的详细任务，并以详细任务文件为准确认其要求、依赖和验证方式。
4. 检查最近一次提交是否包含与该任务直接相关且明确未完成的问题；若存在且会阻塞当前任务，则将其作为当前任务的一部分或作为前置任务处理。
5. 在不引入规避方案的前提下实现该任务所要求的代码与测试改动。
6. 运行与任务直接相关的验证；若需要，补充运行更广的测试、格式化、`clippy` 或构建检查，并修复发现的问题。
7. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
8. 更新对应 `TODO-Px.md` 的完成记录，并将任务标题标记为 `[DONE]`；若任务顺序、标题或依赖发生变化，同步更新 `TODO.md`。仅当阶段计划发生变化时更新 `PLAN.md`。
9. 检查工作区状态，按要求创建一次 git 提交，然后停止，不继续处理下一个任务。

## 约束记录

- 以 `TODO-Px.md` 详细任务文件为任务内容与完成状态的唯一权威来源。
- 只完成一个详细任务；如遇阻塞，仅新增最小必要前置任务并同步索引后提交停止。
- 不接受规避实现、夹具特判或缩小范围的“临时完成”。
- 不记录内部私有推理；本文件仅记录可审阅的执行计划、关键决策与进展。

## 进展日志

- 已创建初始执行计划，下一步读取 `TODO.md` 与详细任务文件以确定当前任务。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认首个未完成详细任务为 `P4-T02`：落地 schema identity、canonical schema pool 与 callable-level facts 壳层。
- 已检查最新提交主题 `[P4-T01R] Sync DONE marker for facts stage boundary review`，未发现与 `P4-T02` 直接相关且需要先补入的未完成问题。
- 下一步：读取 `EFFECT_REFACTOR.md` 对应章节与现有 `crates/scoopc/src/effect_facts/**`、`mir/materialize.rs`、P3/P2 相关 metadata 实现，确认 schema identity、callable key、concrete-op identity 与事实壳层的数据来源。
- 已发现工作区中存在一批与 `P4-T02` 直接相关的未提交实现；当前测试失败点是 `MaterializedEffectFactsBuilder` 在处理 sample fixture 时命中 `UnsupportedEffectTerm { ty: "Any" }`。
- 初步判断：builder 当前把 effect-op 声明实例也当成 callable-level facts 输入，导致把不应进入 callable shell 的 effect-op 根声明带入 schema 构建；同时 `continuation_obj_ty` 目前仅按 `root_fqn` 建 identity，未覆盖同名不同实例的区分需求。
- 修复计划：
  1. 让 callable seed 收集显式跳过 effect-op 根声明实例，仅把真正需要 callable-level facts 的 callable 实例纳入 P4 壳层。
  2. 把 continuation object type identity 改为绑定完整 callable instance 身份，而不是仅绑定 `root_fqn`。
  3. 补充/调整测试，覆盖 effect-op 根声明不会误入 callable facts，以及 continuation object identity 能区分不同实例。
  4. 重新运行 `P4-T02` 指定测试与必要的 smoke/clippy 验证，然后更新 TODO 记录并提交。
- 已完成实现收口：
  1. `effect_facts::builder` 现在会显式跳过 effect-op 根声明与 compiler-owned `scoop.core.Continuation.resume` surface 声明，不再把它们误当成 callable-level facts 输入。
  2. continuation object type identity 已改为绑定完整 `InstanceKey`（模板位置 + type args + allowed-row effect args），避免同名不同实例共用同一个内部类型身份。
  3. builder tests 的 sample fixture 已改成“generic helper + driver 触发 materialized direct-call instances”的形状，确保测试真正覆盖 materialized callable 实例，而不是只看到 surface builtin。
- 已完成验证：
  - `cargo test -p scoopc --no-default-features refactor_effect_schema`
  - `cargo test -p scoopc --no-default-features refactor_continuation_schema`
  - `cargo test -p scoopc --no-default-features refactor_callable_effect_facts_shell`
  - `cargo test -p scoopc --no-default-features refactor_effect_facts_stage`
  - `cargo test -p scoopc --no-default-features materialized_effect_facts_builder_uses_canonical_pass_view_snapshot`
  - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`
  - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
- 已更新 `TODO-P4.md` 与 `TODO.md`：`P4-T02` 现已标记为 `[DONE]` 并补全完成记录；`PLAN.md` 保持不变。
- 下一步：复查最终工作区 diff，按要求把当前所有未提交文件一并提交为一次 git commit，然后停止，不进入 `P4-T02R`。
