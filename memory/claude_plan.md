# 当前执行计划

## 约束
- 以 `TODO.md` 为唯一任务顺序和完成状态来源。
- 只完成第一个未标记 `[DONE]` 的任务，完成后提交并停止。
- 不展开无关历史问题排查；只处理当前任务直接相关或验证中暴露且未被明确排期的失败。
- 不记录内部推理，只记录可审阅的执行计划、关键决策、执行进度和验证结果。

## 步骤
1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并记录任务编号、要求、依赖和验证要求。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置任务写入 `TODO.md`。
3. 阅读与当前任务相关的代码、测试、规格或夹具文件，确定最小正确实现范围。
4. 实现当前任务；若发现无法按规格实现且需要新的具体前置任务，则更新 `TODO.md` 后提交并停止。
5. 按要求运行格式化、lint、相关测试；需要时再运行完整 Rust 测试和完整 fixture 套件，并修复观察到的未排期失败。
6. 更新 `TODO.md`：给完成任务标题加 `[DONE]`，补全完成记录和验证结果；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关变更，提交信息使用任务编号和简明描述。
8. 完成一个任务后停止，不继续下一个任务。

## 进度
- 已读取 `TODO.md`：主索引要求按子计划执行，首个未完成子计划为 `TODO-3.md`。
- 已读取 `TODO-3.md`：第一个未完成任务为 `T3-04G`，主题是收口第七次审查发现的 source-site、ABI/source-signature synthesis、P6 fallback 与 dependency gate 残余缺口。
- 最新提交为 `[T3-04R] Schedule seventh review follow-up`，与当前 `T3-04G` 直接相关，且已由 `TODO-3.md` 明确排期。
- 工作区存在未跟踪 `FACT_REFACTOR.md`，不是本次创建；本次不触碰。
- 初步审查确认当前代码仍存在 `T3-04G` 指向的生产路径残留：P6 HIR source-site bridge、class ctor result/span/base-context fallback、LIR bodyless target key/ABI/source-signature synthesis、effect verifier 逃逸、P6 FQN/string/value-box fallback，以及 dependency gate 漏检。
- 已完成第一轮代码修改：
  - `lir_facts_builder` 删除 bodyless target key / ABI symbol / 空 `Unit` source signature 合成，缺上游 source signature 改为 fail-fast。
  - `mir_stage` 删除静态 `bodyless_signature_root` 清单驱动的 backend source signature 发布。
  - P4 effect facts builder 对缺 source signature 的 bodyless direct 改为 fail-fast，known target 缺 callable facts 不再降级为 bodyless surface；effect verifier 校验 known/candidate target 已发布 callable facts。
  - P6 LLVM stage 不再扫描 `HirFacts.source_sites` 构造 ctor/reflection/continuation bridge；base context 移除 `class_ctor_init_bodies` fallback。
  - MIR/effect-lowered class ctor lowering 改为要求 LIR owner+`SiteId` ctor call-site/init facts；遗留 HIR class ctor/reflection路径改为 fail-fast。
  - 删除若干 P6 generic/overload/FQN 文本恢复路径，包括 top-level generic FQN 合成、缺 ABI symbol 时 `callable_fqn.to_string()`、`mir_direct_call_base_fqn` 及入口名泛型/overload剥离。
- 下一步运行 `cargo fmt`、`python3 tools/dependency_gate.py` 和编译/lint，根据失败继续收口剩余 P6 value-box/dispatch/gate 缺口。
- 已运行 `cargo fmt`。
- 已运行 `cargo check --all-targets`：首次因 `LirClassCtorInitKey` 被误当作带字段结构失败；已改为只消费 LIR ctor call-site/init key，不从 MIR `target_init_class_fqn` 回退或比较 key 内部字段。
- 重新运行 `cargo check --all-targets` 已通过。
- `cargo clippy --all-targets -- -D warnings` 已通过。
- 已补充 dependency gate 守卫，覆盖 LLVM stage HIR source-site 扫描、class ctor init body handoff fallback、LIR/MIR bodyless source-signature/ABI/key 合成，以及 effect verifier known/candidate target 逃逸；重新运行 `python3 tools/dependency_gate.py` 已通过。
- 首次运行 `cargo test --all --all-targets` 时 `scoop` 的 run 测试失败：reachability 把已发布的 global-init root `scoop.core.ARRAY_ELEM_KIND_COMPOSITE` 当作缺 callable/declaration target。已修复 reachability：global-init roots 作为无 callable edge 的显式发布 root 允许通过，调用 target 缺 callable/declaration 仍 fail-fast。
- 复跑 Rust 测试时 `p7_default_pipeline::single_pipeline_runs_receiver_effect_op_cli` 失败：P4 对 `scoop.core.byteLength` 的 bodyless direct target 找不到上游 source signature。已修复 MIR backend source-signature 发布：
  - HIR source-site function target 只有在存在 return type 时才登记到 `seen_source_signatures`，避免 `return_ty=None` 污染去重集合后阻止后续显式发布。
  - 从 HIR declaration facts 补齐 declaration-only source signatures。
  - 为语言内建 String byte substrate 发布 `scoop.core.byteLength` / `scoop.core.getByte` 的 source signature；TypeId 优先从 TypeStore builtin/display 查找，必要时从已发布 String/Int source signatures 推导。
  - 目标测试 `cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_receiver_effect_op_cli` 已通过。
- 后续 `scoopc` lib 测试暴露 declaration-only sysroot targets（`Array.get`、`Array.size`、atomic helper）有 source signature 但无 callable body facts；已恢复 P4 对“source-signature 已发布的 bodyless direct target”的 `BodylessDirect` 表达，缺 source signature 仍 fail-fast。
- 同轮还暴露泛型 class ctor site 的 LIR source class FQN 与 monomorphic layout key不同；已删除 P6 MIR ctor lowering 对 layout fqn 的错误比较，只消费 LIR ctor site 的 target init key。
- `cargo test -p scoopc --lib` 已通过（144 tests）。
- 重新运行 `cargo clippy --all-targets -- -D warnings` 已通过。
- 重新运行 `cargo test --all --all-targets` 已通过。
- 完整 fixture suite 初次暴露若干 P6/ABI 可见性路径仍需要既有 HIR/source fallback 才能维持当前功能：generic HIR direct calls、legacy reflection HIR lowering、class ctor init body base-context fallback、effect facts declaration-only candidate target。已恢复这些兼容路径，同时保留本轮新增的 fail-fast 和 source-signature 修复；随后逐项修复剩余失败。
- 最新验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（1664 checks）均通过。
- 已将 `TODO-3.md` 的 `T3-04G` 标记为 `[DONE]` 并填写完成记录；已将 `TODO.md` 当前活跃任务推进到下一项 `T3-04R`，但本次不会执行该 review。
- 下一步检查 git diff 并提交本次任务变更。

## 本轮 T3-04R 第八次审查进度
- 已定位本轮首个未完成任务为 `TODO-3.md` 的 `T3-04R`，依赖 `T3-04G` 已标记完成。
- 最新提交 `6517049c [T3-04G] Close seventh fallback gaps` 与本轮 review 直接相关；未跟踪 `FACT_REFACTOR.md` 继续视为既有用户文件，不触碰。
- 审查发现 `T3-04G` 后仍有直接阻塞 `T3-04R` 完成的残余：P6 `published_print_callable_fqn` / `published_hir_generic_callable_fqn` concrete FQN 合成与 `fqn.to_string()` 兜底、class ctor init body base-context fallback、MIR `scalar_intrinsic_entry_from_fqn` FQN 推导、value-box/member dispatch 文本恢复路径，以及 dependency gate 对这些实际 helper 的漏检。
- 已在 `TODO-3.md` 中新增最小前置任务 `T3-04H`，将 `T3-04R` 依赖改为 `T3-04H`，并追加八次审查阻塞记录。
- 已在 `TODO.md` 中把当前活跃任务改为 `TODO-3.md` → `T3-04H`。
- 本轮只修改任务清单和进度记录，未修改编译产物；因此不运行 `cargo`/fixture 全量验证，沿用最近 `T3-04G` 提交记录中的全量绿色结果作为代码基线。
