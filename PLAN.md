# Scoop 编译期 Fact 体系统一重构计划

> 生成时间：2026-06-01
> 设计依据：[`EFFECT_INFER.md`](./EFFECT_INFER.md)（effect row 分层与推断）、[`FACT_GAPS.md`](./FACT_GAPS.md)（pipeline fact gap 报告，FG-01–FG-18）
> 任务入口（单一）：[`TODO.md`](./TODO.md)（索引）→ `TODO-1.md` … `TODO-4.md`
> 归档：[`docs/archive/PLAN-2.md`](./docs/archive/PLAN-2.md) / [`docs/archive/TODO-2.md`](./docs/archive/TODO-2.md)（`@ReleaseHook`/`@NoGC`/sync/委托库化；P0–P5-T02A + P5-T02B00 已完成）

## 0. 目标与定位

把整条 compilation pipeline（P1 AST → P2 typed HIR → P3 MIR/materialize → P4 effect facts → P5 LIR → P6 LLVM）的**语义事实**做成：

1. **identity-stable**：每个 declaration/instance/callable/dispatch-target/call-target 都带稳定语义 key（不依赖 FQN 字符串、`::<...>`/`$overload$`/`.$lambda` 约定、source span、本地 `TypeId` 编号）。
2. **self-contained**：每个阶段的 fact artifact 自包含，下游不回看更早的 `LoweredHir` / `MaterializedMir` side table / AST 重新扫描或重建。
3. **完整发布**：上游已知或应能稳定算出的事实（effect row 分层、hidden init effect、call-site target、provenance、step schema、source signature、dynamic-invoke/boundary contract、ABI symbol/layout/closure identity）都进入 handoff，而不是让下游推导。
4. **下游纯消费 + fail-fast**：下游只消费已发布 facts；缺失即报错（verifier），不再用唯一候选、`SignatureFallback`/`DynamicFallback`/`unpublished(...)`/`missing-owner` 兜底掩盖上游漏发。

**驱动场景与验收锚点**：以 effect fact（尤其 owner-`eff` 泛型 + 标准委托 `lazy`/`observable`/`vetoable`）作为端到端驱动与验证——当前 delegate dispatch 的多处不一致（eff-less/eff-Pure 分叉、跨 program step-schema 漂移、`unpublished` dispatch target、invoke shell 悬空）正是上述 fact gap 的集中爆发点。

这是一次**把程序做对**的结构性重构，不是修补测试。遇到与本主线冲突的"下游重建"补丁，**清理掉而非在其上继续修**。

## 1. 现状缺陷（综合 EFFECT_INFER + FACT_GAPS）

- **effect fact 不完整**：HIR 只发 `FunctionEffectContract { allowed_effects, effects_closed }`（`crates/scoopc_hir/src/stage.rs:98`），无 `direct/inferred/published/step` 分层；effect-facts `declared_row` 重算；hidden init effect 在 MIR lowering 重扫 HIR（FG-06）；P4 从 MIR shape 重建 site/block/handled-region（FG-08）、重做 callable value provenance（FG-10）、用 FQN+fallback 定 call-site target/declared row（FG-09）。
- **identity 不稳**：`MonomorphRequest` 缺 stable template key（FG-02）；MIR direct call 只存字符串 FQN（FG-04）；owner-eff dispatch target 从 receiver 重建 eff args（FG-05）；LIR facts 用 `(base fqn, type-arg ids, eff-term ids)` loose signature + `unpublished(...)` 兜底（FG-14）；LLVM call lowering 用 FQN+多级 signature fallback 恢复 root（FG-17）、用 canonical text/FQN 约定重建 symbol/layout/closure path（FG-18）。
- **artifact 不自包含**：materializer 重扫 AST 建 template/body/site catalog（FG-01）、下游遍历 HIR 推 generic direct-call instance（FG-03）；P4 重建 `Index`/`TypeEnv`/dispatch tables（FG-07）；MirFacts 不完整、仍依赖 `MaterializedMir` backend contracts（FG-13）；LirFacts 从 `MaterializedMir` 重发 source signature/dynamic-invoke（FG-15）；LLVM 从 HIR/HirFacts/MaterializedMir 重建 base context（FG-16）；P5 boundary operand/result source contract 从 MIR source slice 恢复（FG-12）；plain local control owner step schema 由 P5 反推（FG-11）。
- **表示不稳定**：effect row 用 `EffectRow{Vec<TypeId>}`，`TypeId` 本地编号；step schema id per-program 局部编号——spurious 实例只要在一个 program 出现就挪动编号，使同一逻辑 callable 在 `late_lowered_program` 与 `abi_program` 拿到不同 step id（本会话 owner-eff 撞墙的直接根因）。
- **expression effect 为语法开后门**：delegated property/operator/loop/computed property 各自特判，而非统一从 canonical semantic facts 推 effect。

共同后果：effect/identity 信息在每个交接面被丢失/默认/重建，规则各处不一致 → 多处分叉。

## 2. 设计

### 2.1 稳定语义 identity keys（display 与 semantic 分离）

- semantic 主键：`StableDefKey`（声明）、`StableTemplateKey`（generic 模板）、`StableInstanceKey`（单态实例：def + canonical type args + canonical effect row args）、`StableCallableKey`/`StableLirCallableKey`（含 body version）、`DispatchTargetKey`、`CallTargetKey`、`AbiSymbolKey`。仓库已有部分（`StableInstanceKey`/`StableTemplateKey`/`StableDefKey`/`StableLirCallableKey`），**本计划是让它们成为 request/IR/fact 的一等字段，而不是下游重建**。
- display identity（FQN、`::<...>`、`$overload$`、`.$lambda`、TypeId display）**只作诊断 anchor**，禁止用于语义匹配。
- type/effect args 用可验证的跨 store 稳定编码（canonical type key + `EffectRowTemplate`），不传裸 `TypeId`。

### 2.2 EffectRowTemplate（effect 维度的稳定表示）

```
EffectRowTemplate { terms: Vec<EffectTerm>, closed: bool }   // closed = Pure!
EffectTerm = Concrete { type_key: StableEffectTypeKey }
           | Param    { owner: StableDefKey, ordinal: u32, name }
```
- 规范排序去重；有确定性 canonical text（跨 program/阶段比较与 instance key 组成的唯一依据）；`substitute(bindings)` 把 `Param`→具体 row。
- 对外发布 facts/key 一律用它，不再暴露裸 `EffectRow{Vec<TypeId>}`（阶段内局部求解可保留，跨阶段/入 key 必转）。

### 2.3 分层 effect rows（EFFECT_INFER）

`declared_surface_row` / `direct_effect_row` / `inferred_surface_row(_template)` / `published_surface_row(_template)` / `step_effect_row`，不再共用 `declared_row`。**method instance 是否把 owner eff 编入身份，由其 published/step row 是否引用 owner eff 决定**（`getValue` Pure→共享 eff-less；`setValue /E`→eff-keyed；class/itable key 仍 eff-aware）。

### 2.4 self-contained artifact 原则

每阶段定义 artifact-complete contract，并用 verifier 断言下游不回看更早 side table；缺 fact 即 fail-fast。

## 3. 各阶段 fact 契约（要发布的内容）

- **HIR/typecheck**：`CallableSourceEffectFacts`（分层 row 模板）；`CallSiteInstanceFact`（site + stable template/instance key + type/eff args，FG-03）；`DispatchCandidateFact`（site + dispatch kind + receiver + stable instance keys + owner eff，FG-05）；`HiddenInitializerEffectFact`（FG-06）；template/body/site-binding inventory（FG-01）；canonical semantic expansion facts（委托/operator/loop/computed property，FG-09 source 面）。
- **MIR/materialize**：`MonomorphRequest` 带 stable key（FG-02）；`CallKind::Direct` 带 resolved callee/instance key（FG-04）；`CallableInstanceEffectFacts {declared/actual/published/step}`；`MirEffectEventFact`/`MirBlockEffectRegionFact`/`MirSiteInventoryFact`（FG-08）；`CallableValueProvenanceFact`/`ResultProvenanceFact`（FG-10）；instance inventory 带 `eff_args`/layout/backend contracts 收口（FG-13）。
- **P4 effect facts**：纯消费上游 identity/site/effect facts；local control owner step schema 设为必发（FG-11）；call-site target/surface 用已发布 fact（FG-09）；不再重建 `Index`/`TypeEnv`/dispatch tables（FG-07）。
- **P5 LIR**：每个 callable/call-target 带 stable `LirCallableKey`/body-version/target key（FG-14）；自带 source signature/dynamic-invoke/`BoundarySourceContract`（FG-12/15）；plain local control fail-fast（FG-11）。
- **P6 LLVM**：只消费 LIR program + LIR facts + type/context artifact；`ExactCalleeBinding`（FG-17）、`AbiSymbolFact`/`LayoutNameFact`/`ClosureIdentityFact`（FG-18）、base context 收口（FG-16）。

## 4. 分批阶段（对应 TODO 文件）

按 FACT_GAPS 的优先级（先 identity → 再 self-contained artifact → 最后删 fallback）+ effect 主线，拆为 4 批；每批末尾独立 review；批次顺序为硬依赖。

- **批 1 — 基础 + 上游 identity（`TODO-1.md`）**：基线清理（回退纯重建 WIP 补丁 + bypass 失败 delegate fixture）；稳定语义 key 体系 + `EffectRowTemplate`；上游（P2/P3）把 stable key + owner eff 写进 `MonomorphRequest`/template-body-site inventory/generic direct-call inventory/`CallKind::Direct`/dispatch candidate。覆盖 FG-01/02/03/04/05/14（表示与上游侧）+ EFFECT_INFER §2.2。
- **批 2 — 完整 fact 发布 + self-contained artifact（`TODO-2.md`）**：HIR 分层 `CallableSourceEffectFacts` + 统一 expression inference + canonical semantic facts（含 hidden init）；MIR `CallableInstanceEffectFacts` + effect-event/site-inventory/provenance facts + backend contracts 收口；P4 消费这些 facts 产出 instance effect facts。覆盖 FG-06/08/09/10/11(发布侧)/12/13/15 + EFFECT_INFER §3/§4。
- **批 3 — 下游纯消费 + 删 fallback / fail-fast（`TODO-3.md`）**：P4 env/dispatch 收口（FG-07）；P5 LIR stable key 消费 + 删 loose-signature/unpublished（FG-14）；P6 LLVM 纯消费（FG-16/17/18）；verifier 禁回看 side table、fallback→fail-fast（cross-cutting #3/#4）。
- **批 4 — effect 语义收口 + 端到端（`TODO-4.md`）**：分层 row 契约与诊断、dispatch effect-ABI 固定（§5）、边界 Pure!（§6）、递归 fixed-point、inference 放宽（§7）；owner-`eff` 委托库化端到端（承接 PLAN-2 P5-T02B0/B/T03）；恢复批 1 bypass 的 fixture/test；全量 golden/fixture/四后端/跨平台回归 + spec 回写。

## 5. Dynamic dispatch ABI

interface/abstract/open/default method 的 `published_surface_row` 是 itable/vtable slot 的 ABI 契约，由 base/interface 固定，实现/子类不得扩展 outward row（可更 Pure，ABI 不变）；dispatch call site 只读静态 receiver slot 契约，不 union 实现。

## 6. 边界与递归

entry/export 完整 published row = `Pure!`；`@NoGC` 禁 eff-row 参数且 direct/published Pure/Pure!；`@Extern` surface row = 声明契约且 Pure/Pure!；递归 SCC fixed-point，不收敛/不可定 → 要求显式 outward row（诊断）。

## 7. Inference 放宽

concrete 函数尾部纯传递 row 可省略（发布推导 published row）；显式 row 必须是完整调用面契约；函数类型/higher-order/无 body API/dispatch slot 仍强制显式；删除"public 省略 effect 必报错"旧规则。

## 8. FACT_GAPS → 批次映射

| FG | 主题 | 批次 |
| --- | --- | --- |
| FG-01 | materializer 重建 template/body/site catalog | 1 |
| FG-02 | MonomorphRequest 缺 stable template identity | 1 |
| FG-03 | generic direct-call instance inventory 下游推导 | 1 |
| FG-04 | MIR direct call 只存字符串 FQN | 1 |
| FG-05 | owner-eff dispatch target 从 receiver 重建 eff args | 1 |
| FG-14 | LIR facts loose signature / unpublished 兜底（identity 侧） | 1（表示）/3（删兜底） |
| FG-06 | hidden init effect 在 MIR 重算 | 2 |
| FG-08 | P4 从 MIR shape 重建 site/block effect event stream | 2 |
| FG-09 | call-site target/declared row 用 FQN+fallback | 2（发布）/3（删 fallback） |
| FG-10 | callable value/closure provenance 在 P4 重做数据流 | 2 |
| FG-11 | plain local control owner StepSchema P5 反推 | 2（必发）/3（fail-fast） |
| FG-12 | P5 boundary operand/result source contract 从 MIR slice 恢复 | 2 |
| FG-13 | MirFacts 不自包含，依赖 MaterializedMir backend contracts | 2 |
| FG-15 | LIR facts 从 MaterializedMir 重发 source signature/dynamic-invoke | 2 |
| FG-07 | P4 重建 Index/TypeEnv/dispatch tables | 3 |
| FG-16 | LLVM 从 HIR/MaterializedMir 重建 base context | 3 |
| FG-17 | LLVM call lowering FQN+fallback 恢复 root | 3 |
| FG-18 | LLVM stable symbol/layout/closure path 重建 | 3 |
| EFFECT_INFER 语义/inference/边界/委托端到端 | — | 4 |

## 9. 验收门禁

- 进入批 2 前：绿色（或已 bypass）基线确立；稳定 key + `EffectRowTemplate` 有 canonical/substitution 单测且与现有 stable_id 对齐；上游 request/IR/dispatch 携带 stable key（不再 span/FQN 兜底）。
- 进入批 3 前：HIR/MIR 发布完整分层 effect facts + site/event/provenance facts；P4 消费这些产出 instance facts；表达式 effect 无语法后门。
- 进入批 4 前：下游无重建残留（verifier/grep 守卫）；P5/P6 只消费 facts；fallback 改 fail-fast；step schema 跨 program 一致。
- 完成批 4 后：全量回归绿、四后端/双平台绿、spec 同步；owner-eff 泛型 + 标准委托端到端工作；批 1 bypass 的 fixture/test 恢复且通过。

## 10. 风险与注意点

- **体量巨大**：横跨全 pipeline，`EffectRowTemplate` + stable key 会改变几乎所有 eff-bearing/泛型 nominal/callable 的 key 与 dump 形态，**大量 golden 需更新**。必须严格分批、每批可独立验证，禁止一次性大爆炸。
- **stable key 与现有体系一致性**：新 key 必须与 `StableInstanceKey`/`stable_instance_fqn`/itable stable id 对齐，否则引入新的不一致。
- **迁移期双源**：每个消费点切换到 facts 后必须立即删旧重建路径（或用门禁断言两者一致后删），不得长期并存。
- **fail-fast 时机**：fallback 改 fail-fast 要按批推进，先发布 fact 再删 fallback，避免中途无法编译。
- **closed（Pure!）语义**：发布时正确区分开放 `Pure` 与闭合 `Pure!`，影响边界与 override。
- **delegate fixture bypass**：批 1 临时 bypass 的 delegate fixture/test 必须在批 4 恢复，不得遗留永久跳过。
