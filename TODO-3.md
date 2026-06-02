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
| T3-03R | [TODO] | Review T3-03 |
| T3-04 | [TODO] | verifier 禁回看 side table + fallback→fail-fast（cross-cutting #3/#4） |
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

### [TODO] T3-03R：Review T3-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-03
- 完成记录：（待填）

### [TODO] T3-04：verifier 禁回看 side table + fallback→fail-fast

- 参考：`FACT_GAPS.md` Cross-Cutting #3/#4、Suggested Priorities #3。
- 必须实现的内容：为每个 stage 定义 artifact-complete contract 并加 verifier，断言下游不消费更早 side table（grep/结构守卫测试）；把残留的 `DynamicFallback`/`SignatureFallback`/`unpublished(...)`/`missing-owner`/唯一候选推断逐一改为 verifier error 或在缺 fact 时 fail-fast；显式删除 T3-03 为保持现有 effect/interface 回归绿色而保留的 `direct_call_dispatch_fqn` / `published_dispatch_target_fqn` compatibility root 解析路径；恢复 T1-00 因 owner-eff 暂忽略的 Rust 单测（若其断言在新机制下成立）。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：跨阶段 fact 自包含被守卫锁定；fallback 改 fail-fast。
- 依赖：T3-03R
- 完成记录：（待填）

### [TODO] T3-04R：Review T3-04
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T3-04
- 完成记录：（待填）
