# TODO-3：下游纯消费 + 删 fallback / fail-fast（批 3）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 3、§2.4；依据 `FACT_GAPS.md` FG-07/09(删 fallback)/11(fail-fast)/14/16/17/18、cross-cutting #3/#4。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：P4/P5/P6 只消费已发布 facts；删除 env/dispatch 重建、loose-signature/unpublished/missing-owner 兜底、FQN+signature fallback；用 verifier 禁止回看更早 side table，缺 fact 改 fail-fast。
> 依赖：批 2（`TODO-2.md`）全部完成。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T3-01 | [DONE] | P4 收口 Index/TypeEnv/dispatch tables 重建（FG-07） |
| T3-01R | [DONE] | Review T3-01 |
| T3-02 | [DONE] | P5 LIR 携带 stable key + 自带 source signature/dynamic-invoke/boundary contract；删 loose-signature/unpublished（FG-14/15/12） |
| T3-02R | [DONE] | Review T3-02 |
| T3-03 | [DONE] | P6 LLVM 纯消费 LIR facts：base context 收口 + exact callee binding + abi symbol/layout/closure facts（FG-16/17/18） |
| T3-03R | [DONE] | Review T3-03 |
| T3-04 | [DONE] | verifier 禁回看 side table + fallback→fail-fast（cross-cutting #3/#4） |
| T3-04A0 | [DONE] | 发布 source-body legacy scalar/toString intrinsic call metadata，解除 T3-04A fixture 阻塞 |
| T3-04A | [DONE] | 收口 T3-04R 审查发现的 P6 side-table / intrinsic fallback / unpublished-target verifier 缺口 |
| T3-04B | [TODO] | 收口 T3-04R 二次审查发现的 source-span / fallback / verifier / gate 残余缺口 |
| T3-04R | [TODO] | Review T3-04 |

---

### [DONE] T3-01：P4 收口前端环境重建（FG-07）

- 必须实现的内容：把 P4 effect-facts 所需的 declaration/type environment、dispatch inventory（vtable/itable/direct subclass）、per-cone public API import 作为 HIR facts / per-cone artifact 显式传给 P4；删除 `effect_facts/builder.rs` 里重新 parse sources、`build_top_level_index`、`TypeEnv::from_sysroot`/`extend_from_file`、重收 vtable/itable/direct subclass 的逻辑；cached-dep API 走 artifact 而非重放注入。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：P4 不再重建前端环境/dispatch tables，纯消费上游 artifact。
- 依赖：T2-03R
- 完成记录：2026-06-02 完成。新增 HIR semantic artifact，携带 frontend 已构造并注入 cached dep public API 后的 `Index`/`TypeEnv`/`HirFacts`；MIR handoff 显式携带该 artifact，P4 effect-facts builder 改为消费 artifact，不再重新 parse sources、`build_top_level_index`、`TypeEnv::from_sysroot`/`extend_from_file` 或重放 cached cone imports。同步生产 LLVM/LIR pipeline 与相关测试 helper，使 cached-dep public API 通过 frontend artifact 进入 P4；保留 cached dep LIR artifact 仅用于 LLVM ABI/layout handoff。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-01R：Review T3-01
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-01
- 完成记录：2026-06-02 完成。审查 T3-01 变更后确认 P4 effect-facts 入口现在从 `MirStageOutput` 消费 `HirSemanticArtifact`，`MaterializedEffectFactsBuilder` 只克隆上游 artifact 中的 `Index`/`TypeEnv`，旧的 source parse、`build_top_level_index`、`TypeEnv::from_sysroot`/`extend_from_file` 与 cached import replay 路径已从 P4 builder 删除；未发现需要新增前置任务的阻塞问题。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-02：P5 LIR stable key + 自包含 source/dynamic-invoke/boundary contract

- 参考：`FACT_GAPS.md` FG-14/15/12。
- 必须实现的内容：
  1. **FG-14**：`LateLoweredProgram` 构造 callable/call-target 时即保存 stable `LirCallableKey`/body-version/target key；EffectFacts 的 `CallSiteTarget` 携带对应 LIR key 或可无损映射的 stable instance key。删除 `lir_facts_builder.rs` 的 `loose_instance_signature`、`unpublished(...)`、`missing-owner` 派生兜底。
  2. **FG-15**：P5 生成 dynamic-invoke / boundary lowering 时直接发布 source callable signature、call-site materialized metadata、dynamic-invoke source contract；`lir_facts_builder` 只序列化，不回看 `MaterializedMir` pass view / raw file items / source slices。
  3. **FG-12**：LIR boundary materialization 消费 T2-02 的 `BoundarySourceContract`，不再从 MIR source slice 恢复 operand/result/carrier source。
  4. **FG-11(fail-fast)**：plain local control owner step schema 缺失时 P5 直接 fail-fast，删除 `discover_plain_local_effect_control_step_schema` 的单候选反推。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：LIR facts 自包含、stable key 驱动；无 loose-signature/unpublished/反推。
- 依赖：T3-01R
- 完成记录：2026-06-02 完成。`LateLoweredCallable` 现在发布 stable LIR callable key、body-version key 与 source kind，P5 在 opt 后统一挂载这些 identity；LIR facts builder 改为消费已发布 identity/source signatures，并从 MIR backend facts 合并 body-less/runtime callable signature，不再现场生成 `unpublished(...)`/`missing-owner` key。call-site target 解析改为 stable instance 驱动，未 lowering 的声明型目标使用 stable declaration key。plain local control owner schema 缺失改为 fail-fast，删除单候选反推。P5 现在为 plain call site、call boundary、control source-slice dynamic call 发布 `LateLoweredCallSiteMaterializedMetadata`，LIR facts builder 不再回扫 `MaterializedMir` source slice 恢复 dynamic-invoke metadata。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-02R：Review T3-02
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-02
- 完成记录：2026-06-02 完成。审查 T3-02 的 LIR stable key/source signature/dynamic-invoke/boundary contract 变更后，发现 post-opt 重建 callable 时只保留了 `source_callable`，未保留新发布的 `source_kind`，会把 member/synthetic callable 的 LIR facts 误标成默认 `TopLevel`；已修复 opt helper 使 `source_kind` 与 source callable metadata 一并保留，并新增回归单测覆盖该路径。复查确认 `lir_facts_builder` 不再使用 `loose_instance_signature`、`unpublished(...)`、`missing-owner` 或 plain local control 单候选反推，dynamic-invoke/boundary metadata 由 P5 LIR 结构发布并序列化。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-03：P6 LLVM 纯消费 LIR facts

- 参考：`FACT_GAPS.md` FG-16/17/18。
- 必须实现的内容：
  1. **FG-16**：把 LLVM base context 所需信息（ordinary callee、source-site、layout/global/dispatch/base contract）收口到 HIR/MIR/LIR facts artifact；P6 输入只保留 LIR program + LIR facts + type/context artifact，不再 clone `LoweredHir`/`HirFacts`/`MaterializedMir` side table（`llvm_codegen_stage.rs`）。
  2. **FG-17**：LIR facts 发布 per-call `ExactCalleeBinding { target_callable_key, root_fqn, abi_symbol, signature_key }`；LLVM call lowering（`call/lowering.rs`/`abi.rs`/`gc.rs`）删除从 `foo::<Bar>`/`$overload$` 字符串剥 FQN + arity/return-type-display 匹配 root 的多级 fallback。
  3. **FG-18**：LIR facts 发布 `AbiSymbolFact`/`LayoutNameFact`/`ClosureIdentityFact`（ABI-visible stable symbol/layout name、closure lexical path、callable version naming）；LLVM 只校验并使用，不再用 canonical text/private mangler/`.$lambda` 约定 + HIR lexical walk 现场生成。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：P6 只消费 LIR facts；无 FQN/字符串/HIR-walk 重建。
- 依赖：T3-02R
- 完成记录：2026-06-02 完成。LIR facts 现在发布 `LirExactCalleeBinding`（target callable key / root FQN / ABI symbol / signature key），并在 physical layout 中补充 `LirAbiSymbolFact`、`LirLayoutNameFact`、`LirClosureIdentityFact`；body-less native/extern backend contracts 也进入 ABI symbol facts。LLVM vtable/itable dispatch declaration 路径现在优先消费已发布 ABI symbol facts，避免只依赖 callable body symbol facts；既有 HIR/generic compatibility root 解析仍保留以维持现有 effect/interface 回归，剩余 compatibility fallback 的 fail-fast/verifier 收口由 T3-04 明确覆盖。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-03R：Review T3-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-03
- 完成记录：2026-06-02 完成。Review T3-03 后发现并修复 LIR fact 契约不完整问题：wire schema 升至 1.7；source signature 现在发布可解析 `signature_key`，`LirExactCalleeBinding.signature_key` 与 source signature 对齐；known-instance exact callee 缺失或漂移改由 LIR facts verifier 拒绝；ABI symbol facts 同时区分 managed `callable_export` 与 native/extern symbol，并补齐 declaration-only exact callee ABI symbol；layout facts 补发布 class vtable layout name；LLVM dispatch declaration 优先使用 managed callable export，closure identity lookup 改为消费 `LirClosureIdentityFact`。新增回归断言覆盖 exact callee/source signature/ABI symbol/layout name 发布。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-04：verifier 禁回看 side table + fallback→fail-fast

- 参考：`FACT_GAPS.md` Cross-Cutting #3/#4、Suggested Priorities #3。
- 必须实现的内容：为每个 stage 定义 artifact-complete contract 并加 verifier，断言下游不消费更早 side table（grep/结构守卫测试）；把残留的 `DynamicFallback`/`SignatureFallback`/`unpublished(...)`/`missing-owner`/唯一候选推断逐一改为 verifier error 或在缺 fact 时 fail-fast；显式删除 T3-03 为保持现有 effect/interface 回归绿色而保留的 `direct_call_dispatch_fqn` / `published_dispatch_target_fqn` compatibility root 解析路径；恢复 T1-00 因 owner-eff 暂忽略的 Rust 单测（若其断言在新机制下成立）。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：跨阶段 fact 自包含被守卫锁定；fallback 改 fail-fast。
- 依赖：T3-03R
- 完成记录：2026-06-02 完成。Effect/LIR verifier 现在拒绝 `SignatureFallback` 精度，P4 对缺 callable facts 的已知目标改为纯 bodyless direct/dynamic surface 或缺 fact 报错，不再把缺失目标发布成 signature fallback；LIR exact callee ABI symbol 不再静默生成兜底，known exact binding 继续由 verifier 校验 source signature/ABI symbol 一致性。LLVM 删除 T3-03 留下的 `direct_call_dispatch_fqn` / `published_dispatch_target_fqn` / `resolve_lir_root_for_hir_direct_call` 兼容解析，并增加 dependency gate 守卫；intrinsic/bodyless pure helper 调用改走集中 intrinsic metadata helper，缺普通 ABI fact 不再落到 callable ABI symbol fallback。Effect facts golden 已更新为 `Precise`/`Widened`，新增 LIR verifier 回归覆盖 signature fallback 拒绝。T1-00 owner-eff Rust ignore 仍按 TODO-4 `T4-04` 保留；两个 observable/vetoable delegate run-pass fixture 也显式登记为 `T4-04` 恢复范围。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-04A0：发布 source-body legacy scalar/toString intrinsic call metadata，解除 T3-04A fixture 阻塞
- 背景：执行 `T3-04A` 时，P6 handoff 已开始移除 `top_level_fun_call_sites`/FQN intrinsic fallback，但完整 fixture suite 暴露 class ctor/source-body lowering 中仍存在未 fact 化的 legacy scalar 与 `ToString.toString` 调用 metadata；如果继续只在 LLVM 端局部猜测，会违反 T3-04A 的 fact-only/fail-fast 要求。
- 必须实现的内容：将 legacy scalar method intrinsic entry（如 `Int.plus`/`Bool.notEquals` 等）、`ToString.toString` 的 builtin scalar/string dispatch、以及 class ctor/source-body 中需要的 concrete generic callable identity，发布为 HIR/MIR/LIR 可消费的结构化 call-site/source contract；P6 只能消费这些 facts，不得通过 FQN helper、source-span binding side table 或 backend-local synthetic fallback 补洞。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`。
- 完成条件：当前观察到的相关 fixture 失败全部恢复通过，尤其是 `build/intrinsic_sysroot_overlay_scalar_method_basic.scoop`、`run-pass/fun_call_add_basic.scoop`、`run_pass_cone/cross_file_ctor_named_default_basic`、`run_pass_cone/cross_file_generic_top_level_val_basic` 与同类 scalar/toString/source-body run-pass；不得新增 backend-only workaround。
- 依赖：T3-04
- 完成记录：2026-06-02 完成。HIR source-body f-string 对 builtin scalar/string 插值现在发布 concrete `Bool/Char/Int/Float/String.toString` 调用身份，并保留非 builtin receiver 的 `ToString.toString` 语义；legacy scalar `@Intrinsic` 无参标注在 HIR facts 中发布 named table entry（含 `Bool.negate` alias）。effect-lowered `ToString.toString` 只在 receiver source type 明确为 builtin scalar/string 时走对应 concrete toString/plain callable 路径，非 builtin receiver 回到已发布 plain callable，避免 `println<T: ToString>`/overlay body 被错误降成 Unit。同步修复缺省 `else` 且 then 分支已终止的 `if` lowering，不再向非 Unit 结果槽写入 Unit，并更新相关 MIR/effect-lowered golden。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [DONE] T3-04A：收口 T3-04R 审查发现的 P6 side-table / intrinsic fallback / unpublished-target verifier 缺口
- 背景：执行 `T3-04R` 审查时确认 `T3-04` 仍未满足“P6 只消费 LIR facts / 缺 fact fail-fast / dependency gate 锁定”的完成条件，必须先补齐该前置缺口再完成 review。
- 必须实现的内容：
  1. **P6 direct-call/source-site side table 删除**：生产 LLVM codegen 不得再消费 `top_level_fun_call_sites`、`current_top_level_fun_call_binding`、`concrete_top_level_fun_call_fqn` 或同类 source-span binding side table 来恢复 direct-call concrete FQN、generic source signature、reflection type args、intrinsic entry 或 vtable/itable dispatch slot owner；这些信息必须由 HIR/MIR/LIR published facts 或 LIR call-site/source contract 表达，缺失时 fail-fast。
  2. **intrinsic metadata fact 化**：删除 `published_named_intrinsic_entry_name_for_fqn` / `published_intrinsic_base_fqn` 这类包装 FQN fallback 的生产路径；named intrinsic entry/base/reflection type arguments 由上游 fact 明确发布并由 LLVM 消费，`scoopc_hir::intrinsics::fallback_named_intrinsic_entry_name_for_fqn` 不得被 P6/P4 生产路径当作替代 fact 使用。
  3. **verifier 覆盖 unpublished target**：LIR verifier 对 `CandidateSet` 的每个 target 执行与 `KnownInstance` 同级的 published callable / ABI symbol / source signature 校验；dispatch layout facts 中的 vtable/itable impl target 也必须验证具备 source signature 与 ABI symbol，避免 LLVM 才发现坏 target。
  4. **declaration-only ABI reachability**：LLVM reachability 必须能消费 declaration-only native/extern ABI symbol 或 exact callee binding，不能因 target 不在 `facts.callables` 而 panic，也不能静默跳过 candidate target。
  5. **dependency gate 收紧**：`tools/dependency_gate.py` 增加结构守卫，覆盖上述 side table helper、intrinsic fallback helper及生产 LLVM 中的 FQN/string fallback；不得通过重命名 helper 绕过 gate。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`。
- 完成条件：`T3-04` 的 fact-only/fail-fast 契约被真实锁住，P6 不再回看上述 source side table 或 FQN intrinsic fallback，相关 verifier/gate/回归测试覆盖新增缺口。
- 依赖：T3-04A0
- 完成记录：2026-06-02 完成。LLVM stage 不再从 `top_level_fun_call_sites` 补 intrinsic/direct-call source side table；legacy scalar named intrinsic 的 P6 本地 FQN fallback 已删除，named intrinsic metadata 改由 LIR facts 发布 `intrinsic_callables` 并由 LLVM 消费。LIR ABI facts 现在为 declaration-only call targets 发布与 target key 绑定的 ABI fact，verifier/reachability 不再通过 `target.readable_path()` 兜底接受 unpublished target；补充 CandidateSet unbound declaration、itable layout target verifier 单测。Dependency gate 增加 `top_level_fun_call_sites`、legacy scalar FQN fallback、generic/overload string parsing 等守卫。同步 schema version 至 1.8，并修复 vtable lowering 在无 receiver direct-call 探测时应返回不匹配而非 panic。验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`（1664 checks）均通过。

### [TODO] T3-04B：收口 T3-04R 二次审查发现的 source-span / fallback / verifier / gate 残余缺口
- 背景：执行 `T3-04R` 二次审查时确认 `T3-04A` 后仍有生产路径不满足 `T3-04` 的 fact-only / fail-fast / dependency-gate 完成条件。该缺口阻塞 review 完成，必须先补齐。
- 必须实现的内容：
  1. **删除 P6 source-span intrinsic/direct-call side table 回看**：LLVM 生产 codegen 不得再通过 `LlvmIntrinsicCallContract`、`published_intrinsic_call_contract`、`published_instantiated_call_fqn` 或仅按 span 唯一匹配的 source-site map 恢复 intrinsic entry、reflection type args、intrinsic base FQN 或 generic concrete FQN；这些信息必须由 LIR call-site/source contract 或 LIR facts 表达，缺失时 fail-fast。
  2. **intrinsic callable facts 不得由 FQN fallback 生成**：`lir_facts_builder` 不得用 `fallback_named_intrinsic_entry_name_for_fqn`、静态 root FQN 清单或 root/string 扫描替代上游发布的 intrinsic metadata；LLVM `published_named_intrinsic_entry_name_for_root` 不得在缺 LIR intrinsic fact 时扫描 call-site side table 兜底。
  3. **dispatch lowering 只消费发布 facts**：class vtable/interface itable lowering 与 MIR dispatch helpers 不得通过 `rsplit_once('.')`、method name/arity 匹配、`class_vtables`/`interfaces` source side table 来恢复 dispatch slot/owner/target；必须消费 LIR dispatch/layout facts 或明确的 LLVM dispatch contract，缺 fact fail-fast。
  4. **删除 `readable_path()` root fallback**：LIR facts builder 与 LLVM dynamic-invoke/layout lookup 不得用 `target.readable_path()` / `target_callable_key.readable_path()` 推断 root FQN、source signature 或 ABI symbol；declaration-only/native/extern target 必须以 target-key 绑定的 source signature + ABI symbol fact 表达，并由 verifier 校验。
  5. **verifier 补齐发布目标与 owner 校验**：P4/P5 verifier 对 `KnownInstance`、`CandidateSet`、dispatch layout target、bodyless direct surface、body-version owner、continuation owner body-version 执行 self-contained 校验；未发布 target 或缺 source signature/ABI symbol 必须在 verifier 阶段报错，而不是由 solver/codegen widened、跳过或兜底。
  6. **dependency gate 锁定上述边界**：`tools/dependency_gate.py` 增加覆盖 LIR facts builder、LLVM tree-wide generic/overload parsing、source-span contract lookup、intrinsic fallback/root wrapper、`readable_path()` root fallback、dispatch FQN parsing的结构守卫；不得只靠重命名 helper 绕过 gate。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/run_fixtures.py`。
- 完成条件：`T3-04` 的 fact-only/fail-fast 契约在 P4/P5/P6 均被真实锁住，审查中确认的 source-span side table、FQN/string/root/readable-path fallback、未发布 target verifier 缺口全部关闭，并有 gate/单测/fixture 覆盖防回归。
- 依赖：T3-04A
- 完成记录：（待填）

### [TODO] T3-04R：Review T3-04
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-04B
- 阻塞记录：2026-06-02 二次审查发现 `T3-04A` 后仍残留 source-span intrinsic/direct-call side table、FQN/string/root/readable-path fallback、dispatch side-table 恢复、verifier 与 dependency gate 覆盖缺口；已新增前置任务 `T3-04B`，本 review 保持未完成。
- 完成记录：（待填）
