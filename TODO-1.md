# TODO-1：基础 + 上游 identity（批 1）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 1、§2.1/§2.2；依据 `FACT_GAPS.md` FG-01/02/03/04/05/14、`EFFECT_INFER.md` §2.2。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：清理与主线冲突的"下游重建"WIP 补丁、确立可绿/可 bypass 的基线；建立稳定语义 identity key 与 `EffectRowTemplate` 基础设施；让上游（P2/P3）的 request/IR/dispatch 携带 stable key + owner eff，而不是下游用 FQN/span/loose-signature 重建。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T1-00 | [TODO] | 基线清理：回退纯重建 WIP 补丁 + bypass 失败 delegate fixture/test |
| T1-00R | [TODO] | Review T1-00 基线与 bypass 范围 |
| T1-01 | [TODO] | 稳定语义 identity key 体系 + `EffectRowTemplate` 基础设施 |
| T1-01R | [TODO] | Review T1-01 表示与稳定性 |
| T1-02 | [TODO] | 上游 identity 贯穿（MonomorphRequest / template-body-site inventory / generic direct-call inventory / `CallKind::Direct` / dispatch candidate 携带 stable key + owner eff） |
| T1-02R | [TODO] | Review T1-02 上游 identity |

---

### [TODO] T1-00：基线清理（回退纯重建 WIP 补丁 + bypass 失败 delegate fixture/test）

- 背景：本会话为推进 owner-eff 委托加了"下游重建"WIP（commit `1bb674df`/`fadf3d7a`/`67a4eb44`）；其中 eff-aware blanket mangle 还动了 golden 但未更新，使全量 fixture 大面积漂移。这些补丁与本计划主线（统一 facts、下游纯消费）冲突，需清理而非叠加。delegate bug 的最终修复在批 4，前置任务期间相关 fixture 暂时 bypass，避免每次 CI build 失败。
- 必须实现的内容：
  1. 回退纯"下游重建"补丁，回到 `4b66dcd7`（P5-T02B00 carrier ABI 完成点）之上的代码状态（保留文档/计划变更）：`git checkout 4b66dcd7 -- crates/`（`4b66dcd7` 之后仅这三个 owner-eff commit 改过 `crates/`，文档由 `440ade4a` 与本批改动负责）。
  2. 对回到该基线后仍失败的 delegate 相关 fixture/test 加 bypass（**待批 4 `T4-04` 恢复**）：fixture 头部加 `// IGNORE-UNTIL-FIX: <reason，注明等批 4 owner-eff 委托端到端恢复>`；owner-eff 相关 Rust 单测（如 `mir::materialize::tests::materialized_mir_mir_materialize_generics_rejects_missing_effect_row_arg`）用 `#[ignore = "..."]` 暂忽略并记录。
  3. 对几个**并发 GC 经常超时**的 fixture 加 bypass（与 owner-eff 无关，timeout 为 55000/59000ms，属既有不稳定问题，**本轮结束后另行安排修复，不在 `T4-04` 恢复清单内**，但需登记）：`tests/fixtures/runtime_gc/gc_language_parallel_alloc_shared_roots.scoop`、`std_sync_backend_parity_immix_major.scoop`、`gc_language_cross_thread_ref_handoff.scoop`、`gc_language_repeated_collect_shared_chain.scoop` —— 头部加 `// IGNORE-UNTIL-FIX: 并发 GC 偶发超时，本轮 Fact 重构后另行安排修复`。
  4. 把"现状 effect/identity 重建点清单"与"回退/bypass 清单（区分 delegate 待批4恢复 / GC 本轮后另行安排）"写入 `./memory/claude_plan.md`，供后续批次消费。
- 必须遵从的约束：不得保留两套并存的重建路径；回退按文件精确，不误伤 carrier ABI（`4b66dcd7`）等正交成果；bypass 仅限 delegate/owner-eff 未完成导致的失败与上述并发 GC 超时，不得借机跳过其它无关用例。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo test --all --all-targets`（仅 bypass 的 owner-eff test 被忽略）；`python3 tools/run_fixtures.py`（仅 bypass 的 delegate fixture 被 SKIP，其余全绿）。
- 完成条件：CI 全绿（owner-eff/delegate 失败已 bypass 并登记待恢复）；清单写入 memory。
- 依赖：无
- 完成记录：（待填）

### [TODO] T1-00R：Review T1-00 基线与 bypass 范围
- 必须实现的内容：复核回退干净（无 eff-aware mangle/重建残留、carrier ABI 保留）、bypass 仅限 owner-eff/delegate、清单准确。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T1-00
- 完成记录：（待填）

### [TODO] T1-01：稳定语义 identity key 体系 + `EffectRowTemplate` 基础设施

- 参考：`PLAN.md` §2.1/§2.2；现有 `StableInstanceKey`/`StableTemplateKey`/`StableDefKey`/`StableLirCallableKey`。
- 必须实现的内容：
  1. `EffectRowTemplate { terms, closed }` + `EffectTerm = Concrete{type_key} | Param{owner: StableDefKey, ordinal, name}`（落点与 stable_id 同层）：规范排序去重、canonical text、`substitute(bindings)`、`Pure!`(closed) 判定；单元测试覆盖 canonical 稳定性 / substitution / closed / 与具体 effect 类型 key 对齐。
  2. 梳理并补齐语义 identity key 面（`StableDefKey`/`StableTemplateKey`/`StableInstanceKey` 含 effect 维度、`DispatchTargetKey`/`CallTargetKey`/`AbiSymbolKey`）：统一 canonical text，明确"display identity（FQN/`::<...>`/`.$lambda`/TypeId display）仅诊断 anchor，不用于语义匹配"。
  3. type/effect args 用可验证跨 store 稳定编码（canonical type key + `EffectRowTemplate`），提供与现有 `EffectRow{Vec<TypeId>}` 的双向转换（阶段内局部求解仍可用 `EffectRow`，跨阶段/入 key 必转 template）。
- 必须遵从的约束：对外发布的 facts/key 不再用裸 `TypeId`/span/FQN-字符串语义匹配；新表示必须与现有 stable_id 对齐，不引入第二套不一致。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。
- 完成条件：稳定 key + `EffectRowTemplate` 可用、有 canonical/substitution 单测、与现有 stable_id 一致。
- 依赖：T1-00R
- 完成记录：（待填）

### [TODO] T1-01R：Review T1-01 表示与稳定性
- 必须实现的内容：复核无 `TypeId`/span 泄漏、canonical/substitution 正确、display 与 semantic identity 分离、与现有 stable_id 对齐。
- 验证：`cargo test --all --all-targets`
- 依赖：T1-01
- 完成记录：（待填）

### [TODO] T1-02：上游 identity 贯穿（P2/P3）

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-01/02/03/04/05/14。
- 必须实现的内容：
  1. **FG-02**：`MonomorphRequest`（`crates/scoopc_hir/src/monomorph.rs`）携带 `StableTemplateKey`/declaration identity 与 type/eff args 的稳定编码；materializer seed（`materialize/seed.rs`）按 stable key 精确匹配，删除 `(fqn, decl_file)` 唯一性兜底与裸 re-intern。
  2. **FG-01/03**：HIR facts 发布 materializer-ready 的 template/body/site-binding inventory 与 per-call-site `CallSiteInstanceFact { source_site, template_key, stable_instance_key, type_args, eff_args }`；materializer 只消费 fact，不再扫 AST/HIR 重建 lookup key（`materialize/templates.rs`/`hir_calls.rs`）。
  3. **FG-04**：MIR `CallKind::Direct`（`crates/scoopc_mir/src/mir/mod.rs`）附 resolved callee definition key + concrete `StableInstanceKey`，FQN 仅 display；materializer dispatch/rewrite 删除 receiver/arg/result-type 反推（`materialize/dispatch.rs`/`rewrite.rs`）。
  4. **FG-05**：HIR/P3 dispatch contract 发布 `DispatchCandidateFact { site, dispatch_kind, receiver_ty, stable_instance_keys }`，owner effect args 纳入 canonical target identity；删除从 receiver 重建 owner eff 的逻辑。
  5. **FG-14（表示侧）**：让 LIR callable/call-target 能直接由上游 stable key 无损映射（兜底删除留批 3）。
- 必须遵从的约束：owner-eff 泛型的必需基础（`is_generic` 含 `eff_param`、MIR `has_eff_param`、boundary upcast 的 args+eff 比较）按新表示重新落地；不恢复 blanket eff-aware mangle、不按名称特判。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（bypass 的 delegate 仍 SKIP）。
- 完成条件：上游 request/IR/dispatch 携带 stable key + owner eff；materializer/dispatch 不再 span/FQN/receiver 重建。
- 依赖：T1-01R
- 完成记录：（待填）

### [TODO] T1-02R：Review T1-02 上游 identity
- 必须实现的内容：复核 request/MIR call/dispatch candidate 携带 stable key、owner eff 进入 canonical identity、上游重建点已删。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T1-02
- 完成记录：（待填）
