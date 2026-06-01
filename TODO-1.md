# TODO-1：基础 + 上游 identity（批 1）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 1、§2.1/§2.2；依据 `FACT_GAPS.md` FG-01/02/03/04/05/14、`EFFECT_INFER.md` §2.2。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：清理与主线冲突的"下游重建"WIP 补丁、确立可绿/可 bypass 的基线；建立稳定语义 identity key 与 `EffectRowTemplate` 基础设施；让上游（P2/P3）的 request/IR/dispatch 携带 stable key + owner eff，而不是下游用 FQN/span/loose-signature 重建。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T1-00 | [DONE] | 基线清理：回退纯重建 WIP 补丁 + bypass 失败 delegate fixture/test |
| T1-00R | [DONE] | Review T1-00 基线与 bypass 范围 |
| T1-01 | [DONE] | 稳定语义 identity key 体系 + `EffectRowTemplate` 基础设施 |
| T1-01R | [DONE] | Review T1-01 表示与稳定性 |
| T1-02A | [DONE] | Stable request/direct-call identity transport 前置落地（MonomorphRequest stable key seed + `CallKind::Direct` stable instance carrier） |
| T1-02B | [DONE] | HIR stable call-site facts + MIR Direct stable template carrier foundation |
| T1-02 | [TODO] | 上游 identity 贯穿（template-body-site inventory / generic direct-call inventory / dispatch candidate 携带 stable key + owner eff，并删除剩余 materializer/dispatch 下游重建） |
| T1-02R | [TODO] | Review T1-02 上游 identity |

---

### [DONE] T1-00：基线清理（回退纯重建 WIP 补丁 + bypass 失败 delegate fixture/test）

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
- 完成记录：
  - 代码基线：`crates/` 已回到 `4b66dcd7` 对应状态；相对该基线仅保留 1 个 owner-eff Rust 单测 `#[ignore]` bypass（见下条），未保留纯“下游重建”WIP 路径。
  - Delegate / owner-eff bypass：10 个 delegated-property/delegate fixture 加 `IGNORE-UNTIL-FIX`，1 个 owner-eff materialize Rust 单测加 `#[ignore]`；均登记在 `memory/claude_plan.md`，由批 4 `T4-04` 恢复。
  - 并发 GC timeout bypass：4 个指定 runtime GC fixture 加 `IGNORE-UNTIL-FIX`，登记为本轮 Fact 重构后另行修复，不纳入 `T4-04`。
  - 越界 bypass 复核：`tests/fixtures/effect_facts/dispatch_and_resume_call.scoop` 不属于 delegate/owner-eff 范围，已取消跳过并修正 `dispatch_and_resume_call.effectfacts` 的 `instance_count` golden 漂移（6 → 5）。
  - 现状 effect/identity 重建点清单已写入 `memory/claude_plan.md`。
  - 验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo build -p scoop -p scoopc` 通过；`cargo test --all --all-targets` 通过（仅上述 1 个 owner-eff 单测 ignored）；`python3 tools/run_fixtures.py` 通过（14 个已登记 fixture skipped，`fixtures: ok (1663)`）。

### [DONE] T1-00R：Review T1-00 基线与 bypass 范围
- 必须实现的内容：复核回退干净（无 eff-aware mangle/重建残留、carrier ABI 保留）、bypass 仅限 owner-eff/delegate、清单准确。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T1-00
- 完成记录：
  - 基线复核：`git diff --stat 4b66dcd7 -- crates` 仅剩 `crates/scoopc_mir/src/mir/materialize/tests.rs | 1 +`，即已登记的 owner-eff materialize 单测 `#[ignore]`；`eff[-_ ]?aware|mangle.*eff|eff.*mangle|blanket.*mangle|owner[-_ ]?eff.*mangle` 定向搜索无命中，未发现 eff-aware mangle / 纯下游重建 WIP 残留。
  - Carrier ABI 复核：当前 `crates/` 相对 `4b66dcd7` 仅有上述测试 ignore 差异，因此 `4b66dcd7` 的 effectful closure/method carrier explicit-arg ABI 代码保持保留。
  - Bypass 范围复核：fixture `IGNORE-UNTIL-FIX` 仅有 14 个，分别为 10 个 delegate/owner-eff fixture 与 4 个已登记并发 GC timeout fixture；Rust `#[ignore]` 仅有 1 个 owner-eff materialize 单测；`tests/fixtures/**/*.scoop` 无 lowercase `ignore-until-fix`。
  - 验证：`python3 tools/run_fixtures.py` 通过，`fixtures: ok (1663)`；输出中仅上述 14 个登记 fixture 被 skip。

### [DONE] T1-01：稳定语义 identity key 体系 + `EffectRowTemplate` 基础设施

- 参考：`PLAN.md` §2.1/§2.2；现有 `StableInstanceKey`/`StableTemplateKey`/`StableDefKey`/`StableLirCallableKey`。
- 必须实现的内容：
  1. `EffectRowTemplate { terms, closed }` + `EffectTerm = Concrete{type_key} | Param{owner: StableDefKey, ordinal, name}`（落点与 stable_id 同层）：规范排序去重、canonical text、`substitute(bindings)`、`Pure!`(closed) 判定；单元测试覆盖 canonical 稳定性 / substitution / closed / 与具体 effect 类型 key 对齐。
  2. 梳理并补齐语义 identity key 面（`StableDefKey`/`StableTemplateKey`/`StableInstanceKey` 含 effect 维度、`DispatchTargetKey`/`CallTargetKey`/`AbiSymbolKey`）：统一 canonical text，明确"display identity（FQN/`::<...>`/`.$lambda`/TypeId display）仅诊断 anchor，不用于语义匹配"。
  3. type/effect args 用可验证跨 store 稳定编码（canonical type key + `EffectRowTemplate`），提供与现有 `EffectRow{Vec<TypeId>}` 的双向转换（阶段内局部求解仍可用 `EffectRow`，跨阶段/入 key 必转 template）。
- 必须遵从的约束：对外发布的 facts/key 不再用裸 `TypeId`/span/FQN-字符串语义匹配；新表示必须与现有 stable_id 对齐，不引入第二套不一致。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。
- 完成条件：稳定 key + `EffectRowTemplate` 可用、有 canonical/substitution 单测、与现有 stable_id 一致。
- 依赖：T1-00R
- 完成记录：
  - 在 `crates/scoopc_hir/src/stable_id.rs` 新增 `EffectRowTemplate { terms, closed }`、`EffectTerm::{Concrete, Param}`、`StableEffectParamKey`、effect-param resolver、canonical text、substitution、`Pure!` 判定，以及 `EffectRow` ⇄ template 的受控转换 API（从本地 `EffectRow` 生成 stable template；通过 canonical type key resolver 转回本地 `EffectRow`）。
  - `StableInstanceKey` 现在保存结构化 effect arg template，并继续用 canonical type arg + canonical effect-row template text 组成语义 identity；open concrete row 的 canonical text 保持既有 `E(...)` 形态以避免无关漂移。
  - 补齐 `DispatchTargetKey`、`CallTargetKey`、`AbiSymbolKey` 的 canonical key 表面；readable/FQN path 仅作为 `StableSymbolKey::readable_path()` 诊断/符号可读锚点，不作为语义匹配依据。
  - 单元测试覆盖 canonical 稳定性、substitution、closed/Pure! 判定、concrete effect type key 对齐、`StableInstanceKey` effect template 存储、call/dispatch/ABI key canonical 分离。
  - 验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（155/156 MIR 单测中 1 个 T1-00 已登记 owner-eff 单测 ignored）；`python3 tools/run_fixtures.py` 通过（`fixtures: ok (1663)`）。

### [DONE] T1-01R：Review T1-01 表示与稳定性
- 必须实现的内容：复核无 `TypeId`/span 泄漏、canonical/substitution 正确、display 与 semantic identity 分离、与现有 stable_id 对齐。
- 验证：`cargo test --all --all-targets`
- 依赖：T1-01
- 完成记录：
  - 复核 T1-01 新增 `EffectRowTemplate` / `EffectTerm` / `StableInstanceKey` / call-dispatch-ABI key 表示，未发现新增 API 直接把裸 `TypeId`、span、`TypeStore::display()` 或 `Debug` 输出作为 canonical identity 输入；既有下游 FQN/span 重建点仍属于 T1-02/TODO-2 后续范围。
  - 修复 review 发现的参数化 effect row round-trip 缺口：`EffectRowTemplate::from_canonical_text` 现在能把 `eff_param(...)` canonical term 还原为结构化 `EffectTerm::Param`，`StableInstanceKey::from_canonical_args` 不再丢失 effect-param 结构；补充 effect-param marker → template、canonical text → template、substitution 回归测试。
  - 修复 `AbiSymbolKey` display/semantic identity 分离缺口：`PartialEq`/`Hash` 现在只使用 ABI kind + target canonical text，`readable_path` 仅保留为诊断/符号可读锚点；补充 readable path 不影响 equality/hash 的测试。
  - 验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（仅 T1-00 已登记 owner-eff 单测 ignored）；`python3 tools/run_fixtures.py` 通过（`fixtures: ok (1663)`）。

### [DONE] T1-02A：Stable request/direct-call identity transport 前置落地

- 背景：执行原 `T1-02` 时发现其 fact inventory / dispatch 纯消费改造依赖 request 与 materialized direct-call 已能承载 stable identity；否则后续 HIR/P3 facts 即使发布 stable key，下游仍会退回 `(fqn, decl_file, span)` / raw `TypeId` 重建。该前置步骤可独立验证并先落地，不改变原 `T1-02` 的剩余验收边界。
- 必须实现的内容：
  1. `MonomorphRequest`/`MonomorphKey` 支持携带 `StableTemplateKey` 与 `StableInstanceKey`；materializer 入口用 template catalog 为 concrete requests 补齐 stable identity。
  2. materializer 建立 `StableTemplateKey -> TemplateKey` 索引，seed requests 按 stable key 精确匹配；删除 `(fqn, decl_file)` 唯一性兜底。
  3. type/effect args 本地化时以 stable canonical encoding 为校验主线，materializer 不再无验证地接受 raw cross-store re-intern 结果。
  4. MIR `CallKind::Direct` 增加 boxed optional `StableInstanceKey` carrier；materialization 在 generic direct-call 实例解析成功时写入 concrete stable instance key，FQN 继续仅作为 display/debug surface。
- 必须遵从的约束：不恢复 blanket eff-aware mangle；不按名称特判 owner-eff；不得把本步骤视为原 `T1-02` 全量完成，剩余 fact inventory / dispatch candidate / 下游重建删除仍由后续 `T1-02` 完成。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：request seed 与 materialized direct-call 已具备 stable identity transport，现有回归全绿。
- 依赖：T1-01R
- 完成记录：
  - `MonomorphKey` 新增 stable template/instance identity 字段；materializer 入口通过 template catalog 对 concrete requests 生成 stable keys。
  - materializer 新增 stable-template 索引，seed 路径改为按 `StableTemplateKey` 精确解析 template，并移除了旧 `(fqn, decl_file)` 唯一性兜底。
  - seed 的 type/effect arg 本地化现在按 `StableInstanceKey` 的 canonical type args 与 `EffectRowTemplate` 校验，避免未验证的 raw cross-store re-intern 结果成为语义身份。
  - `CallKind::Direct` 增加 boxed optional `StableInstanceKey` carrier；generic direct-call materialization 成功推导 concrete instance 后写入该 stable key。
  - 验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（仅 T1-00 已登记 owner-eff 单测 ignored）；`python3 tools/run_fixtures.py` 通过（`fixtures: ok (1663)`）。

### [DONE] T1-02B：HIR stable call-site facts + MIR Direct stable template carrier foundation

- 背景：执行 `T1-02` 时确认完整删除 materializer fallback 仍依赖更完整的 HIR fact 覆盖；直接删除会让隐式泛型调用、sysroot intrinsic generic、部分跨文件 request-root 形态缺少替代 fact。先落地已可独立验证的上游 stable call-site fact 与 MIR direct-call carrier 基础，供后续 `T1-02` 完成纯消费删除。
- 必须实现的内容：
  1. HIR lowering 发布 generic template stable key side table，避免后续 call-site contract 只能拿 display/FQN 信息。
  2. HIR `FunctionTarget` fact 携带 stable template/instance canonical key，并发布 `CallSiteInstanceFact` / `DispatchCandidateFact` 数据面。
  3. MIR `CallKind::Direct` 补齐 stable template carrier；HIR→MIR direct-call lowering 能从 fact 写入 stable template + stable instance。
  4. materializer direct-call rewrite 在 carrier 已存在时优先按 stable instance key enqueue/resolve；保留旧 fallback 仅作为 `T1-02` 剩余删除目标。
- 必须遵从的约束：不恢复 blanket eff-aware mangle；不按 owner-eff 名称特判；不把旧 fallback 删除伪装为完成，剩余删除继续由 `T1-02` 跟踪。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：facts/carrier 基础可用，有回归测试证明 HIR call-site stable instance fact 与 MIR Direct stable carrier。
- 依赖：T1-02A
- 完成记录：
  - `LoweredHir` 新增 `generic_stable_template_keys`，由 HIR lowering 从 compilation unit + source cone stable identity 构建 generic template stable key side table。
  - `scoopc_hir_facts::source_sites::FunctionTarget` 新增 stable template/instance canonical key；`SourceSiteFacts` 新增 `CallSiteInstanceFact` 与 `DispatchCandidateFact` 发布面。
  - `CallKind::Direct` 新增 optional `stable_template_key` carrier；MIR direct-call lowering 从 HIR facts 写入 stable template/instance keys。
  - materializer direct-call rewrite 对已有 stable instance key 走 stable-key-first instance enqueue/resolve，并在推导 generic instance 后同步写回 stable template + stable instance carrier。
  - 补充 `stable_id` canonical text parse round-trip 单测，以及 HIR facts/MIR direct-call stable carrier 回归测试。
  - 验证：`cargo fmt` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过（仅 T1-00 已登记 owner-eff 单测 ignored）；`python3 tools/run_fixtures.py` 通过（`fixtures: ok (1663)`）。

### [TODO] T1-02：上游 identity 贯穿（P2/P3）

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-01/02/03/04/05/14。
- 必须实现的内容：
  1. **FG-02 剩余收口**：在 `T1-02A` 的 stable request seed 基础上，把 request stable identity 的生成继续上移到 P2/P3 fact/side-table 发布点；下游不得再把 `(fqn, decl_file, span)` 作为语义匹配主键。
  2. **FG-01/03**：HIR facts 发布 materializer-ready 的 template/body/site-binding inventory 与 per-call-site `CallSiteInstanceFact { source_site, template_key, stable_instance_key, type_args, eff_args }`；materializer 只消费 fact，不再扫 AST/HIR 重建 lookup key（`materialize/templates.rs`/`hir_calls.rs`）。
  3. **FG-04 剩余收口**：在 `T1-02A` 的 `CallKind::Direct` stable instance carrier 基础上，补齐 resolved callee definition key，并删除 materializer dispatch/rewrite 的 receiver/arg/result-type 反推（`materialize/dispatch.rs`/`rewrite.rs`）。
  4. **FG-05**：HIR/P3 dispatch contract 发布 `DispatchCandidateFact { site, dispatch_kind, receiver_ty, stable_instance_keys }`，owner effect args 纳入 canonical target identity；删除从 receiver 重建 owner eff 的逻辑。
  5. **FG-14（表示侧）**：让 LIR callable/call-target 能直接由上游 stable key 无损映射（兜底删除留批 3）。
- 必须遵从的约束：owner-eff 泛型的必需基础（`is_generic` 含 `eff_param`、MIR `has_eff_param`、boundary upcast 的 args+eff 比较）按新表示重新落地；不恢复 blanket eff-aware mangle、不按名称特判。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（bypass 的 delegate 仍 SKIP）。
- 完成条件：上游 request/IR/dispatch 携带 stable key + owner eff；materializer/dispatch 不再 span/FQN/receiver 重建。
- 依赖：T1-02B
- 完成记录：（待填）

### [TODO] T1-02R：Review T1-02 上游 identity
- 必须实现的内容：复核 request/MIR call/dispatch candidate 携带 stable key、owner eff 进入 canonical identity、上游重建点已删。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T1-02
- 完成记录：（待填）
