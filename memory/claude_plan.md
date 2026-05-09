## 执行计划

说明：我不会写出逐字逐句的内部思维过程，但会在这里持续维护足够详细的执行摘要、决策依据与步骤计划，供你检查进度。

### 初始计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 检查最近一次提交信息，确认是否有与该任务直接相关且明确未完成的问题需要并入当前任务或作为前置任务。
3. 阅读当前任务涉及的说明、依赖、验证要求，以及必要的实现文件与测试。
4. 如任务可直接完成：实现该任务，补充或调整测试，并运行任务要求的验证命令。
5. 如存在阻塞当前任务的真实缺陷或缺失能力：先在 `TODO.md` 中以最小必要粒度加入前置任务、调整依赖顺序，并记录阻塞原因；仅在阶段计划发生变化时更新 `PLAN.md`。
6. 完成后更新 `TODO.md`：将该任务标题改为 `[DONE]`，补全完成记录。
7. 整理并提交本次变更，提交信息使用当前任务号，随后停止，不继续下一个任务。

### 执行约束

- 只处理 `TODO.md` 中第一个未完成任务。
- 不用变通方案掩盖规范不匹配；若遇到阻塞，先建前置任务。
- 仅在阶段/依赖结构变化时更新 `PLAN.md`。
- 在执行过程中，如计划变化或关键步骤完成，将继续更新本文件。

### 当前任务识别

- 首个未完成任务：`P8-T03a`（`TODO-P8.md`）。
- 最近提交为 `P8-T03aa`，内容直接修复了执行 `P8-T03a` 时暴露的 nominal upcast blocker；`TODO-P8.md` 已把它记录为本任务前置，当前无需再新增前置任务。
- 当前工作树除本文件外，已存在与 `P8-T03a` 直接相关的未提交改动：
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/mod.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  这些改动看起来是在继续完成 `P8-T03a`，需要纳入本次任务一起验证并提交。

### 当前执行步骤

1. 审核 `P8-T03a` 的要求与现有未提交实现，确认默认单文件 LLVM 入口是否已经完全切到 refactor stage。
2. 审核默认 LLVM 单测与显式历史 helper 的边界，必要时继续改写测试 helper 或断言。
3. 运行 `P8-T03a` 指定的定向验证，若失败则直接修复。
4. 若定向验证通过，再更新 `TODO-P8.md`，将 `P8-T03a` 标记为 `[DONE]` 并补全完成记录。
5. 提交当前任务相关的全部未提交文件并停止。

### 当前进展

- 已确认默认单文件 `emit_minimal_main_*` / `build_minimal_main_module*` 与 `scoopc` 单文件 artifact 入口都已指向 refactor LLVM stage。
- 已确认尚存的 `*_from_lowered_hir` / `*_from_materialized_lowered_hir` helper 主要是显式历史/对照入口；本次继续补一条更窄的 raw-materialized helper，以避免默认 helper 再替历史对照测试背锅。
- 第一次定向回归发现两个缺口：
  1. `direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome` 被改名后，TODO 里的 `--exact` 命令只跑到 0 个测试；已恢复原测试名。
  2. `boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint` 仍断言旧式 boxed payload GEP；已改为断言 refactor IR 中“从 explicit frame reload 并重建 aggregate，再发布 Step payload”的现行语义。
- 还发现 `RootCallableSelector::Callable` 在非测试构建会触发 `dead_code` 警告；已在实现侧消除此警告，避免后续 `clippy -D warnings` 被卡住。

### 本次接手说明

- 本次从现有未提交状态继续完成 `P8-T03a`，不会另起范围，也不会跳到 `P8-T04`。
- 接下来优先确认最近提交是否只是在 `P8-T03aa` 收尾，以及当前工作树未提交改动是否全部属于 `P8-T03a`。
- 若这些改动与任务目标一致，就在其基础上继续做最小修正、跑完任务要求的定向验证、更新 `TODO-P8.md` 并提交。

### 新发现与调整

- 已完成 `P8-T03a` 指定的核心 LLVM 定向回归，当前未提交代码在 stage 迁移与默认/历史 helper 分层上是成立的。
- 但执行任务要求的 smoke 命令时，发现 `cargo run -p scoopc -- tests/fixtures/build/emit_llvm_basic.scoop` 仍失败于 `driver_cli`：当前 CLI 还要求显式 `--emit-llvm` / `--emit-obj`，与 `TODO-P8.md` 中“默认单文件 artifact 入口”契约不一致。
- 这属于 `P8-T03a` 直接覆盖的 public single-file entry 缺口，不新增前置任务；改为在本任务内最小修复 `crates/scoopc/src/driver_cli.rs` / `crates/scoopc/src/bin/scoopc.rs` 及相关测试，使 bare `<file>` 默认为 LLVM IR，`--obj <file>` 走 object 路径，同时继续保持入口走 refactor LLVM stage。

### 本次复核收尾

- 当前 `HEAD` 已为 `[P8-T03a] Split project and virtual-cone LLVM entry contracts`；本次继续执行的原因是 `TODO-P8.md` 已记为 `[DONE]`，但总索引 `TODO.md` 仍未同步该状态，因此按规则 `P8-T03a` 仍是首个未完成任务。
- 已重新运行 `P8-T03a` 的定向验证矩阵：`cargo test -p scoopc driver_cli`、`cargo test -p scoop build_context_keeps_bare_file_input_as_virtual_cone_inside_cone_root`、`cargo test -p scoop build_frontend_`、`cargo test -p scoop no_hidden_legacy_fallback_for_default_refactor_build_output`、5 条关键 `scoopc` LLVM 单测，以及将 `scoopc <file>` / `scoopc --obj <file>` 输出重定向到 `/var/.../opencode` 的两条 smoke；结果全部通过。
- 已同步 `TODO.md` 将 `P8-T03a` 标记为 `[DONE]`，并在 `TODO-P8.md` 补充本次复核与复跑记录。
- 下一步：提交 `TODO.md`、`TODO-P8.md` 与本文件，然后停止，不进入 `P8-T04`。

### 2026-05-10 当前轮次：P8-T04

- 已重新读取 `TODO.md` / `TODO-P8.md`。当前首个未完成任务已变为 `P8-T04`。
- 最近提交仅为 `[P8-T03a] Sync task completion records`，未声明与 `P8-T04` 直接相关的额外未完成事项；当前工作树干净，因此本轮直接执行 `P8-T04`。

### P8-T04 执行计划

1. 按任务要求顺序运行完整回归矩阵，先从 `cargo test --all` 开始，定位首个失败点。
2. 若发现失败：仅修复 refactor 主线、中立共享模块、runtime/GC 基础设施或纯新主线测试/文档问题；禁止恢复任何 legacy 分支或 fallback。
3. 修复后重跑受影响命令，直到整套矩阵全部通过。
4. 执行任务要求的 residual `legacy` / `async` / `Task` 搜索，并把命中分类为“历史文本/负向测试/非 effect 语义”或“必须继续清理的残留”。
5. 至少再完整重跑一遍 P8-T04 矩阵，确认不是局部修复后的未汇总状态。
6. 更新 `TODO.md` / `TODO-P8.md` 完成记录，并提交本轮变更；不进入 `P8-T04R`。

### P8-T04 当前进展

- 已确认当前执行范围：只做 `P8-T04`，不提前进入 review 任务。
- 已执行首个完整矩阵命令 `cargo test --all`，结果失败；当前不需要新增前置任务，因为失败点都属于本任务要求的“删除旧主线后最终回归修复”范围。
- 首轮失败按性质暂分三类：
  1. 测试基础设施问题：`effect_refactor_pipeline::llvm_codegen_stage` 的 stage-run 计数在全量并行测试下被其它测试污染，随后触发 `Mutex` poison 连锁失败。
  2. 旧 helper / 旧断言残留：若干 LLVM 单测仍假定 `*_from_lowered_hir` 可处理 `handle`，或仍假定旧函数名/旧 wrapper/旧 descriptor 形状。
  3. 真实代码缺口：至少已看到 effect-typed plain closure adapter 缺少 dynamic-invoke layout、effectful funptr carrier ABI drift 等前端/ABI 问题。
- 下一步先修第 1 类共享问题，并对第 2/3 类失败继续做定向回归与最小修复。

### P8-T04 阻塞更新（本轮结论）

- 在修掉 stage 计数污染、默认 helper 断言漂移、FunPtr dynamic carrier、float `!=` predicate、direct-HIR vtable fallback、`CLayout(aligned)` descriptor 发布、以及多条 LLVM regression 之后，`cargo test -p scoopc --lib` 已收敛到只剩 2 条失败：
  1. `llvm::tests::object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary`
  2. `llvm::tests::top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary`
- 这两条不是简单断言漂移，而是同一个真实 blocker：默认 refactor helper `__scoop_refactor_direct_invoke__a_helper` 仍通过 `scoop_effect_outcome_consume_current` / `scoop_effect_handler_stack_swap_top` 包装 object/top-level hidden init。
- 复核定位：
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs::lower_class_ctor_boundary(...)`
  - `crates/scoopc/src/llvm/codegen/object_init.rs` 中的 object/top-level init access
- 这已经构成 `P8-T04` 的明确前置缺口；因此已把它前插为 `P8-T03ab`，并把 `P8-T04` 依赖更新为 `P8-T03a, P8-T03ab`。
- 本轮到这里停止，不继续硬冲 `P8-T04`。
