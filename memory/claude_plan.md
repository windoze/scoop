## 当前执行计划

说明：按安全与协作要求，这里记录可审计的执行计划、关键决策与进度更新，不写入内部推理细节。

1. 读取 `TODO.md`，把它当作索引使用。
2. 按 `TODO.md` 中引用的详细任务文件顺序检查 `TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md` 等，定位第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；如果是，将其视为当前任务的一部分，或在对应详细任务文件中补充为前置任务。
4. 阅读当前任务的详细要求、约束、验证方式与依赖，必要时补充本文件中的实施步骤。
5. 在不绕过规范、不引入临时性 workaround 的前提下，直接实现该任务；如果遇到真实阻塞，按要求只添加最小必要前置任务，并同步 `TODO.md`。
6. 运行与当前任务直接相关的测试、格式化、lint 与构建检查，至少包括必要的 Rust 测试与 `cargo clippy --all-targets -- -D warnings`（若适用范围过大，则先从任务相关最小集合开始，必要时再扩大）。
7. 更新详细任务文件中的完成记录，并将该任务标题显式改为 `[DONE]`；如果索引中的标题、顺序或状态需要同步，则更新 `TODO.md`。
8. 仅在阶段级依赖、顺序或完成标准发生变化时更新 `PLAN.md`；不把它当作日常执行日志。
9. 检查工作区变更，避免回退非本次任务相关改动；按仓库既有提交风格创建一次 git 提交，然后停止，不进入下一个任务。

## 进度更新

- 已创建初始计划文件，下一步开始读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`，确认当前首个未完成详细任务为 `P6-T02m`：发布 continuation surface-resume -> owner dispatch contract。
- 已检查最新提交信息：最近一次提交仅为 `Update plan`，未发现与 `P6-T02m` 直接相关、需要并入当前任务的未完成事项。

## 当前任务细化

1. 检查 `P6-T02m` 依赖的现有实现与已完成前置任务，重点阅读：
   - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs`
   - `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs`
   - `crates/scoopc/src/effect_facts/builder.rs`
   - `crates/scoopc/src/effect_lowered/ir.rs`
   - 相关测试。
2. 确认当前 handoff 中已有的 surface-resume schema、dispatch-source inventory、continuation object layout 绑定与 ABI query 缺口。
3. 以最小改动实现 authoritative dispatch contract 与 object-side lookup contract，并加入缺失/歧义 fail-fast 校验。
4. 增补或更新定向测试，覆盖共享 schema、多来源 dispatch、以及缺失/歧义拒绝路径。
5. 运行当前任务要求的测试与必要的格式/lint 检查；若失败则先修复。
6. 更新 `TODO-P6-part2.md`、必要时同步 `TODO.md`，然后按仓库风格提交一次 git commit 并停止。

## 当前实现决定

- 当前最终采用的 LLVM query 设计：
  - 对非 `Unreachable` schema 统一发布唯一 owner trampoline contract，作为 shared surface-resume symbol 继续进入 owner-specific lowering 的 authoritative target；
  - 若来源为 `ContinuationObjectMethod`，再额外发布 `method_targets[]`，显式保留同一 owner/object 下的一个或多个 reachable method lookup（`interface_id + field_index + case_tag + vtable_index`）；
  - 若来源为 `ResumeBoundaryOnly` / `HandleContinuationBinderOnly` / `OwnerTrampolineMixed`，owner trampoline 上直接公开 resume-site 列表或 handle-binder route 列表；
  - 仅在出现多 object 共享同一 schema、缺失 method target、或 lookup 漂移时 fail fast，而不是把选择逻辑留给 `P6-T03` 现场扫描或猜测。

## 已完成关键步骤

- 已在 `crates/scoopc/src/llvm/codegen/effect_refactor/types.rs` 增加 `surface-resume dispatch` 查询类型与 `RefactorAbiQuery` 查询入口。
- 已在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 物化 schema -> object-method / owner-trampoline dispatch contract，并加入缺失 lookup、缺失 method target、以及多 target 歧义的 fail-fast 校验。
- 已新增针对 object-method、handle-binder-only、multi-site resume-boundary-only 和歧义拒绝路径的单元测试。
- 已运行并通过：`cargo test -p scoopc refactor_llvm_surface_resume_layout`、`cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`。

## 新发现的 blocker

- 在执行更广范围验证 `cargo test -p scoopc refactor_llvm_` 时，`refactor_llvm_dynamic_invoke_query_resolves_fun_value_unit_contract` 失败。
- 失败原因不是旧代码偶发问题，而是当前新设计假设过强：实际存在同一 `ContinuationSchemaId` 在同一个 continuation object 上对应多个 reachable internal method target（测试里报出 `ko0 ri0::c0` 与 `ko0 ri1::c1`）。
- 这说明 `P6-T02m` 不能把 object-method source 简化成“唯一 method target”；必须把同 schema 的多 method dispatch 作为显式 published contract 表达出来，否则会把真实允许的 contract 误判为歧义。
- 下一步：检查该 fixture 的 late-lowered shape，扩展 query 设计，让 object-method source 能 authoritative 地发布“同 schema -> 多 reachable method target + object-side lookup/selector”，然后重新跑完整验证。

## Blocker 处理结果

- 已通过 `dynamic_fallback_widening.scoop` dump 确认：同一 schema 的多 reachable method target 是真实 contract，而不是脏数据。
- 已把 `surface-resume dispatch` query 改为：
  - 对非 `Unreachable` schema 一律发布唯一 owner trampoline contract；
  - 对 `ContinuationObjectMethod` 额外发布 `method_targets[]`，允许同一 owner/object 下保留多个 reachable method lookup；
  - 仅在出现多 object 共享同一 schema、缺失 method target、或 lookup 漂移时 fail fast。

## 收尾状态

- 代码改动、验证和 TODO 文档同步均已完成。
- 已完成并通过的关键验证：
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_llvm_surface_resume_dispatch_layout`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
  - `cargo fmt --all`
- 已更新：`TODO-P6-part2.md`、`TODO.md`、`memory/claude_plan.md`。
