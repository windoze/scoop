# Scoop：Effect Fact 统一重构计划

> 生成时间：2026-06-01
> 设计依据：[`EFFECT_INFER.md`](./EFFECT_INFER.md)（effect row 推断与 facts 分层提案）
> 被取代/归档：[`PLAN-2.md`](./PLAN-2.md)（`@ReleaseHook` + `@NoGC` + `scoop.sync`/委托库化计划，其中 P0–P5-T02B00 已完成；P5 剩余委托库化任务承接到本计划之后）
> 任务清单：[`TODO.md`](./TODO.md)

## 0. 目标与定位

把 effect 当作**一等的、完整的、稳定表示的、统一发布的编译期 fact**贯穿 typecheck → HIR → effect-facts → MIR/materialization → LIR → codegen 全链路；下游阶段**只消费已发布的 effect facts，不再从 receiver 类型、声明、mangle、TypeId 等基础信息各自重建**。

这是一次**把程序做对**的核心重构，不是为了让某几个 fixture 通过。目标是消除当前 effect 处理的结构性缺陷，从而根除 owner-`eff` 泛型/委托路径里反复出现的多处不一致（eff-less / eff-Pure / eff-Raise 分叉、跨 program step-schema 编号漂移、dispatch target 解析失败、invoke shell 声明却无 body 等）。

## 1. 现状缺陷（已核实）

- **HIR 发布的 effect fact 不完整**：`FunctionEffectContract` 只有 `allowed_effects` + `effects_closed`（`crates/scoopc_hir/src/stage.rs:98`），没有 `direct_effect_row` / `inferred_surface_row` / `published_surface_row` / `step_effect_row` 的分层，调用点/override/跨 cone/dispatch 拿不到统一的 callee surface 契约。
- **effect-facts 重算而非消费**：`declared_row` 从 `raw_fun` 声明重新计算（`effect_facts/builder.rs` `declared_effect_row`），owner-`eff` / dispatch 的 effect 还要从 **receiver 类型重建**（`callable_owner_context`）。
- **下游各自重建 instance 身份**：itable `materialize_member_impl_fqn_for_owner`、lir-facts `target_callable_key`、codegen step schema 选择，分别用 mangle / overload 索引 / instance-key 的 TypeId 重建 method 身份与 effect，规则彼此不一致。
- **effect row 表示不稳定**：`EffectRow { terms: Vec<TypeId> }` 依赖 per-program 的 `TypeId`，不可跨 program/阶段稳定比较；step schema id 又是 per-program 局部编号。spurious 实例只要在一个 program 出现就会挪动编号，导致同一逻辑 callable 在两个 program（`late_lowered_program` 与 `abi_program`）拿到不同 step id。
- **expression effect 推断为语法开后门**：delegated property / operator / loop / computed property 等各自特判，而不是统一从 canonical semantic facts 推 effect。

这些缺陷的共同后果：**effect 信息在每个边界被丢失/默认/重建**，于是 owner eff 时有时无、Pure 方法被错误地按 owner eff 分裂、不同 program 的 step schema 集合不一致。当前 `P5-T02B0`/`P5-T02B` 的 owner-eff 工作以及若干 WIP commit 都是在"下游重建"层打补丁，治标不治本。

## 2. 设计：分层 effect rows + 稳定 row 表示

### 2.1 分层 effect rows（术语，来自 `EFFECT_INFER.md`）

- `declared_surface_row`：源码显式写出的函数尾部 `/ Row`；写了就是该 callable 的 published 调用面契约。
- `direct_effect_row`：函数体**直接 perform** 的 effect；用于判定 public concrete 函数能否省略 row。
- `inferred_surface_row`：从函数体 + 表达式语义推导出的**完整 outward** row（direct + 传递）。
- `published_surface_row`：调用者、override 检查、跨 cone API、dispatch ABI 消费的 outward 契约。显式声明则 = 声明 row（并校验 `inferred ⊆ declared`）；省略则 = `inferred_surface_row`。
- `step_effect_row`：后端 effect lowering / state machine / step schema 选择需要覆盖的内部 effect 全集（可含被本地 `handle` 吸收的 perform、resume runtime error、hidden init effect），可大于 `published_surface_row`。

这些 row 不再共用一个 `declared_row` 名字。

### 2.2 稳定 row 表示（一等、可序列化、可替换；本计划的核心基础设施）

新增统一类型（落点：`scoopc_types` / `scoopc_ids` 与 stable_id 同层），**所有对外发布的 effect facts 一律用它**，不再对外暴露裸 `EffectRow{Vec<TypeId>}`：

```
EffectRowTemplate {
    terms: Vec<EffectTerm>,   // 规范排序、去重
    closed: bool,             // true = Pure!（禁止 effect 多态扩展）
}

EffectTerm =
  | Concrete { type_key: StableEffectTypeKey }            // 具体 effect 名义类型（如 scoop.core.Raise<Int>），用稳定 type key
  | Param    { owner: StableDefKey, ordinal: u32, name }  // owner 声明的第 ordinal 个 eff 参数；name 仅供诊断/可读
```

- **稳定 key**：`Concrete` 用与 nominal 一致的 stable type key（canonical text / stable type id），**绝不用 `TypeId`**；`Param` 用 `owner` 的 `StableDefKey` + owner-relative `ordinal`（ordinal 比 name 更稳定，name 留作诊断）。这是 `EFFECT_INFER.md` §"Row Template Representation" 的硬约束。
- **canonical text**：`EffectRowTemplate` 有确定性、可序列化的 canonical text，作为跨 program/阶段比较与 instance key 组成的**唯一依据**（替换现在散落各处的 mangle/TypeId 比较）。
- **substitution**：`substitute(bindings: (StableDefKey, ordinal) -> EffectRowTemplate)`，在 materialization 把 `Param` 替换为具体 row；幂等、可组合。
- **closed 语义**：`Pure!` = `terms 空 + closed`；entry/export/`@NoGC`/`@Extern` 边界要求 closed Pure。
- 阶段内的具体类型检查仍可用现有 `EffectRow{Vec<TypeId>}` 做局部求解，但**进入 fact / instance key / 跨阶段传递时必须转换为 `EffectRowTemplate`**。

### 2.3 instance 身份中的 effect

method instance 是否把 owner eff 编入身份，由该 method 的 **`published_surface_row`（及 `step_effect_row`）是否引用 owner eff** 决定，而不是"凡是 owner-eff class 的方法都带 owner eff"：

- `getValue(thisRef, prop): V`（Pure，row 不引用 E）→ instance 身份**不含** owner eff，所有 eff 实例化共享同一 `getValue::<...>` body。
- `setValue(...): Unit / E`（row 引用 E）→ instance 身份**含** owner eff（`setValue::<.., eff R>`），每个 eff 一个实例。
- class（类型描述符 / itable）身份仍 eff-aware（因为 setValue slot 随 eff 不同）。

这正是当前 whack-a-mole 的根因修复点：blanket eff-aware mangle 把 Pure 的 getValue 也按 owner eff 分裂，造成重复实例与编号漂移。

## 3. 各阶段 facts 契约

### 3.1 HIR/typecheck：source-level callable facts

```
CallableSourceEffectFacts {
    declared_surface_row: Option<EffectRowTemplate>,
    direct_effect_row: EffectRowTemplate,
    inferred_surface_row_template: EffectRowTemplate,
    published_surface_row_template: EffectRowTemplate,
    row_is_closed: bool,
    inference_status,   // 是否需要显式 row / SCC 未收敛等
}
```

- `check_required_effects_for_fun_decl` 的收集结果从"只用于本函数报错"改为**发布本 callable 的 surface facts**。
- public concrete 函数省略 `/ Row` 且 `direct_effect_row` 非 Pure → 报错要求显式完整调用面 row。
- interface（含 default body）/ abstract / open method 的 `published_surface_row_template` 必须来自**显式** ABI 契约（dispatch slot 不可推断）。

### 3.2 MIR/materialization：instance-level callable facts

```
CallableInstanceEffectFacts {
    declared_surface_row: Option<EffectRowTemplate>,
    actual_surface_row: EffectRowTemplate,      // 实例化 body 推出的真实 outward row
    published_surface_row: EffectRowTemplate,   // 调用者可见（显式则用声明实例化 + 校验 actual ⊆ published）
    step_effect_row: EffectRowTemplate,         // 供 effect lowering / step schema 选择
}
```

- instance key（callable 与 nominal）用稳定 row 表示编入 effect 成分；getValue/setValue 按 §2.3 区分。
- step schema 由 `step_effect_row` 的 canonical 形态决定，**跨 program 一致**（不再 per-program 重建编号）。

## 4. 统一 expression-level effect inference + canonical semantic facts

- 每个 expr 有 `expr_surface_row`，按统一规则 union 子表达式 + callee `published_surface_row`，按 handler 规则移除被本地处理且不 outward 的 effect。
- delegated property / operator / loop / computed property / constructor-init 等在 typecheck/HIR 发布**统一的 canonical semantic facts**（如"该 expr 的 core operation 是 `getValue` call"），effect inference 只消费这些 facts，**不按语法/名称特判、不开后门**。这同时实现 P5 想要的"属性委托对编译器透明"。

## 5. Dynamic dispatch ABI

- interface / abstract / open / default method 的 `published_surface_row` 是 itable/vtable slot 的 **ABI 契约**，由 base/interface 固定，实现/子类不得扩展 outward row（可更 Pure，但 ABI 不变）。
- dispatch call site 只读**静态 receiver 类型上的 slot 契约**，不 union 各实现的实际 row；step schema/ABI 由 slot 固定，避免不同实现产生不同 callable ABI。

## 6. 边界与递归

- entry/export 要求完整 `published_surface_row` 为 `Pure!`；`@NoGC` 禁 eff-row 参数且 direct/published 均 Pure/Pure!；`@Extern` 无 body，surface row = 声明契约且强制 Pure/Pure!。这些边界消费推导后的完整 surface facts。
- 递归 SCC 用 fixed-point 推导 `inferred_surface_row`；effect-polymorphic recursion / 未解析动态分派 / 跨 cone body 缺失 / row 变量不可定 → 要求显式 outward row（诊断策略，非语法限制）。

## 7. Inference 放宽（与 facts 完整化一并落地）

- concrete 函数尾部**纯传递性** row 可省略，省略时发布推导出的 `published_surface_row`；显式 row 必须是完整调用面契约（不能只列 direct 增量）。
- 函数类型、higher-order 参数/返回/字段/property/typealias、带 eff-row 参数的函数类型、无 body 的 API、所有进入动态分派 ABI 的 method：**必须显式写完整 row**。
- 删除"public 省略 effect 必报错"的旧规则，替换为本方案的 direct-effect 判定。

## 8. 与现有 owner-eff 打地鼠补丁的关系（清理而非叠加）

新 facts 落地时，**清理**那些纯属"下游重建"的临时补丁，不在其上继续修补（项目未发布，可自由 breaking）：

- 保留并纳入新方案的**真·基础支持**：`@ReleaseHook`/`@NoGC`/sync/委托库化（PLAN-2 P0–P5-T02A）、effectful 带参 closure/method carrier ABI 修复（`P5-T02B00`，commit `4b66dcd7`，与本重构正交的真 bug）。
- 审计并**回退/重写**的"重建补丁"（owner-eff WIP，commit `1bb674df`/`fadf3d7a`/`67a4eb44` 中的相应部分）：`callable_owner_context` 的 receiver-eff 回退、`dispatch_target_owner_eff_args` 注入、blanket eff-aware `mangle_nominal_fqn_with_eff` 用于所有 method、`collect_concrete_class_targets` 的 ad-hoc 跳过、lir-facts loose-instance 解析等——改为由统一 facts 驱动。其中 owner-eff 泛型的**必需**基础（`is_generic` 含 eff_param、MIR `has_eff_param` 过滤、boundary upcast 的 args+eff 比较）在新方案下仍需要，按新表示重新落地。

## 9. 分阶段实施与验收门禁

- **E0** 稳定 row 表示基础设施（`EffectRowTemplate`/`EffectTerm`/canonical text/substitution）+ 单元测试。
- **E1** HIR/typecheck 发布 `CallableSourceEffectFacts`（含分层 row 模板，稳定表示）。
- **E2** 统一 expression-level effect inference + canonical semantic facts（含委托/operator/loop/computed property）。
- **E3** 调用点 / override / dispatch ABI 改为消费 published facts；dispatch slot 固定 row。
- **E4** MIR/materialization 发布 `CallableInstanceEffectFacts`；instance 身份按 §2.3 区分 owner-eff 依赖；step schema 由 `step_effect_row` 跨 program 一致。
- **E5** 下游（effect-facts / LIR / codegen）改为消费 instance facts，**删除所有重建逻辑与对应 WIP 补丁**。
- **E6** 边界（entry/export/`@NoGC`/`@Extern`）+ 递归 fixed-point。
- **E7** Inference 放宽（省略传递 row）+ 旧规则删除。
- **E8** spec / fixtures / goldens 全量更新；四后端/跨平台回归；owner-eff 委托端到端（承接 PLAN-2 的 P5 剩余目标）。

每阶段后紧跟独立 review 任务；阶段间门禁：上一阶段发布的 facts 被下游实际消费且无重建残留、相关 fixtures 与单测绿、`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt` 干净。

## 10. 风险与注意点

- **范围与体量**：横跨 typecheck/HIR/effect-facts/MIR/LIR/codegen 全链路，且 `EffectRowTemplate` 会改变所有 eff-bearing nominal/callable 的 key 与 dump 形态，**大量 golden 需更新**。必须分阶段、每阶段可独立验证，避免一次性大爆炸。
- **稳定 key 与现有 stable_id 一致性**：`EffectTerm::Concrete` 的 type key、`Param` 的 `StableDefKey` 必须与现有 `StableInstanceKey`/`stable_instance_fqn`/itable stable id 体系对齐，否则又会引入新的不一致。
- **closed（Pure!）语义**：发布 facts 时必须正确区分开放 `Pure` 与闭合 `Pure!`，影响边界校验与 override 兼容。
- **递归/未解析分派的 fallback**：fixed-point 不收敛时的显式-row 要求必须有清晰诊断，不能静默默认 Pure。
- **迁移期一致性**：E1–E5 之间，旧重建路径与新 facts 不能并存产生双源；每步要么完全切换该消费点，要么用门禁断言两者一致后再删旧路径。
