# Effect Fact 统一重构 TODO

> 生成时间：2026-06-01
> 计划基线：[`PLAN.md`](./PLAN.md)
> 设计依据：[`EFFECT_INFER.md`](./EFFECT_INFER.md)
> 归档：[`PLAN-2.md`](./PLAN-2.md) / [`TODO-2.md`](./TODO-2.md)（`@ReleaseHook`/`@NoGC`/sync/委托库化；P0–P5-T02A + P5-T02B00 已完成）
> 当前状态：effect fact 重构阶段 E0–E8 全部 `[TODO]`；PLAN-2 的 P5 委托库化剩余目标承接到本文件末尾，依赖 effect 重构完成。
> 行号说明：下文行号以引用时文件状态为准；实现时优先按文件路径、函数名、fixture 名定位。

## 总原则

- `PLAN.md` 是当前执行计划基线；实现时若发现阶段边界或 fact 契约需变化，必须先回写 `PLAN.md` 再调整本文件。
- 任务按 E0 → E8 顺序推进，**不跨阶段并行实现**。每个实现任务后必须紧跟一个独立 review 任务（编号 + `R`）。
- **目标是把 effect 处理做对，不是修补测试**：下游阶段只消费已发布的 effect facts，不得从 receiver 类型 / 声明 / mangle / `TypeId` 重建 effect。遇到与本主线冲突的旧"下游重建"补丁，**清理掉而不是在其上继续修补**。
- **effect row 对外发布一律用稳定表示**（`EffectRowTemplate`，基于 stable def/type key，禁用 `TypeId`），见 `PLAN.md` §2.2。
- 完成任务后更新本文件状态（`[TODO]`→`[DONE]`）与完成记录；只有标题带 `[DONE]` 才算完成。
- 安全/边界约束不可削弱：entry/export/`@NoGC`/`@Extern` 的 Pure!/Pure 约束、`@ReleaseHook` 安全闭环，迁移后必须逐项保持。
- 所有 runtime/codegen 改动须保持 `baseline`/`immix`/`hosted`/`minimal` 四后端可编译可回归。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| E0-T00 | [TODO] | 审计现有 effect 处理与 owner-eff WIP 补丁，清理纯"下游重建"补丁，确立绿色基线 |
| E0-T00R | [TODO] | Review E0-T00 基线与清理范围 |
| E0-T01 | [TODO] | `EffectRowTemplate` 稳定 row 表示基础设施（term/canonical text/substitution/closed） |
| E0-T01R | [TODO] | Review E0-T01 表示与稳定性 |
| E1-T01 | [TODO] | HIR/typecheck 发布 `CallableSourceEffectFacts`（分层 row 模板，稳定表示） |
| E1-T01R | [TODO] | Review E1-T01 source facts |
| E2-T01 | [TODO] | 统一 expression-level effect inference + canonical semantic facts（含委托/operator/loop/computed property） |
| E2-T01R | [TODO] | Review E2-T01 表达式 effect 推断统一性 |
| E3-T01 | [TODO] | 调用点 / override / dynamic dispatch 改为消费 published facts；dispatch slot 固定 row |
| E3-T01R | [TODO] | Review E3-T01 消费面与 dispatch ABI |
| E4-T01 | [TODO] | MIR/materialization 发布 `CallableInstanceEffectFacts`；instance 身份按 published row 区分 owner-eff 依赖；step schema 跨 program 一致 |
| E4-T01R | [TODO] | Review E4-T01 instance facts 与 step schema 一致性 |
| E5-T01 | [TODO] | 下游（effect-facts/LIR/codegen）消费 instance facts，删除全部重建逻辑与对应 WIP 补丁 |
| E5-T01R | [TODO] | Review E5-T01 重建清零 |
| E6-T01 | [TODO] | 边界（entry/export/`@NoGC`/`@Extern`）消费完整 facts + 递归 SCC fixed-point |
| E6-T01R | [TODO] | Review E6-T01 边界与递归 |
| E7-T01 | [TODO] | Inference 放宽：concrete 函数省略纯传递 row；删除旧 public-omitted 必报错规则 |
| E7-T01R | [TODO] | Review E7-T01 inference 放宽 |
| E8-T01 | [TODO] | 全量 golden/fixture/四后端/跨平台回归 + spec 回写 |
| E8-T01R | [TODO] | Review E8-T01 收口 |
| P5-DLG-01 | [TODO] | 在新 effect facts 之上完成 lazy/observable/vetoable 库化端到端（承接 PLAN-2 P5-T02B0/P5-T02B） |
| P5-DLG-01R | [TODO] | Review P5-DLG-01 委托库化 |
| P5-DLG-02 | [TODO] | 委托同步回归 + 零硬编码守卫扩展（承接 PLAN-2 P5-T03） |
| P5-DLG-02R | [TODO] | Review P5-DLG-02 回归与守卫 |

## 阶段间验收门禁

- 进入 E1 前：绿色基线确立（四后端可编译、`cargo clippy --all-targets -- -D warnings` 干净、全量 fixture 已知失败集被精确记录）；`EffectRowTemplate` 表示稳定、有 canonical text 与 substitution 单测、与现有 stable_id 对齐。
- 进入 E3 前：HIR 已发布完整分层 source facts，且 effect inference 统一从 canonical semantic facts 消费（无语法后门）。
- 进入 E5 前：调用点/override/dispatch 已消费 published facts；MIR 已发布 instance facts，step schema 跨 program 一致。
- 进入 E6 前：下游无任何 effect 重建残留（grep 守卫）；owner-eff/委托路径 codegen 不再出现 eff-less/eff-Pure 分叉、invoke-shell 悬空。
- 完成 E8 后：全量回归绿、四后端/双平台绿、spec 同步；owner-eff 泛型 + 标准委托端到端工作。

---

## E0：基线与稳定表示

### [TODO] E0-T00：审计现有 effect 处理与 owner-eff WIP 补丁，清理纯重建补丁，确立绿色基线

- 背景：本会话为推进 owner-eff 委托加了若干"下游重建"补丁（commit `1bb674df`/`fadf3d7a`/`67a4eb44`），与本计划主线（统一 facts、下游只消费）冲突；当前 HEAD 因 eff-aware mangle 动了 golden 且未跑全量，基线可能为红。
- 必须实现的内容：
  1. 梳理 effect 处理现状并记录到 `./memory/claude_plan.md`：各阶段发布的 effect fact、各处"重建"点（至少 `crates/scoopc_hir/src/stage.rs` `FunctionEffectContract`、`effect_facts/builder.rs` `callable_owner_context`/`declared_effect_row`/`dispatch_target_owner_eff_args`、`itable.rs` `materialize_member_impl_fqn_for_owner`/`collect_concrete_class_targets`、`hir/mod.rs` `mangle_nominal_fqn_with_eff`、`lir_facts_builder.rs` `target_callable_key`/`loose_instance_signature`）。
  2. 区分三类：(a) 与新主线正交、保留（`@ReleaseHook`/`@NoGC`/sync/委托库化 PLAN-2 成果、carrier ABI `4b66dcd7`）；(b) owner-eff 泛型**必需基础**、按新表示重写（`is_generic` 含 `eff_param`、MIR `NominalMetadata.has_eff_param`、boundary upcast 的 args+eff 比较）；(c) 纯"下游重建"补丁、**回退**（`callable_owner_context` receiver-eff 回退、`dispatch_target_owner_eff_args`、blanket `mangle_nominal_fqn_with_eff` 用于所有 method、`collect_concrete_class_targets` ad-hoc skip、lir-facts loose-instance 解析）。
  3. 回退 (c) 类补丁，使分支回到**编译通过 + clippy 干净 + 全量 fixture 仅剩 PLAN-2 已记录的委托相关失败**的绿色基线；(b) 类基础若回退后导致 owner-eff 泛型完全无法编译，可暂时保留并在完成记录中标注"待 E4 按新表示重写"。
- 必须遵从的约束：不得保留两套并存的 effect 重建路径；回退要按 commit/hunk 精确，不误伤正交成果。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py`（记录失败集为基线）。
- 完成条件：绿色（或失败集被精确记录且仅限承接委托任务）基线确立；现状与清理清单写入 memory。
- 依赖：无
- 完成记录：
  - （待填）

### [TODO] E0-T00R：Review E0-T00 基线与清理范围
- 必须实现的内容：复核清理清单分类正确（正交保留 / 必需重写 / 纯重建回退）、无双源残留、基线失败集精确。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E0-T00
- 完成记录：（待填）

### [TODO] E0-T01：`EffectRowTemplate` 稳定 row 表示基础设施

- 参考：`PLAN.md` §2.2；现有 stable_id（`StableInstanceKey`/`stable_instance_fqn`/`StableDefKey`）。
- 必须实现的内容：
  1. 在与 stable_id 同层（`scoopc_ids` 或 `scoopc_types`）新增 `EffectRowTemplate { terms: Vec<EffectTerm>, closed: bool }` 与 `EffectTerm = Concrete{ type_key } | Param{ owner: StableDefKey, ordinal, name }`，term 规范排序+去重。
  2. 实现 canonical text（确定性、可序列化、可比较）、`substitute(bindings)`（`Param`→具体 row，幂等可组合）、与 `Pure!`(closed) 判定。
  3. 与现有 stable type key 对齐：`Concrete.type_key` 用 nominal 一致的稳定 type key（非 `TypeId`）。
  4. 单元测试覆盖：canonical 稳定性（同逻辑 row 跨构造一致）、substitution（含嵌套/多 param）、closed 区分、与具体 effect 类型 key 对齐。
- 必须遵从的约束：对外发布的 effect facts/key 不得再用裸 `EffectRow{Vec<TypeId>}`；阶段内局部求解可保留 `EffectRow`，但跨阶段/入 key 必须转 `EffectRowTemplate`。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`。
- 完成条件：稳定表示可用、有 canonical text 与 substitution、单测绿。
- 依赖：E0-T00
- 完成记录：（待填）

### [TODO] E0-T01R：Review E0-T01 表示与稳定性
- 必须实现的内容：复核稳定性（无 `TypeId`/span 泄漏）、canonical/substitution 正确、与 stable_id 对齐。
- 验证：`cargo test --all --all-targets`
- 依赖：E0-T01
- 完成记录：（待填）

---

## E1：HIR/typecheck source-level facts

### [TODO] E1-T01：发布 `CallableSourceEffectFacts`

- 参考：`PLAN.md` §3.1；`EFFECT_INFER.md` §72-108、§168-218；`crates/scoopc_hir/src/typecheck/expr/stmt.rs` `check_required_effects_for_fun_decl`、`crates/scoopc_hir/src/stage.rs` `FunctionEffectContract`。
- 必须实现的内容：
  1. 定义并发布 `CallableSourceEffectFacts { declared_surface_row, direct_effect_row, inferred_surface_row_template, published_surface_row_template, row_is_closed, inference_status }`（用 `EffectRowTemplate`）。
  2. 把 `check_required_effects_for_fun_decl` 收集结果从"只报错"改为"发布 facts"。
  3. public concrete 函数省略 `/ Row` 且 `direct_effect_row` 非 Pure → 报错要求显式完整调用面 row；interface(含 default)/abstract/open method 的 `published_surface_row_template` 必须来自显式契约。
  4. member method 的 owner eff 以 `Param{owner, ordinal}` 出现在模板里。
- 验证：`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；新增/更新 typecheck fixtures。
- 完成条件：HIR 发布完整分层 source facts，调用点诊断可基于 `published_surface_row_template`。
- 依赖：E0-T01R
- 完成记录：（待填）

### [TODO] E1-T01R：Review E1-T01 source facts
- 必须实现的内容：复核分层 row 正确、显式/省略规则、dispatch slot 强制显式、模板稳定表示。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E1-T01
- 完成记录：（待填）

---

## E2：统一 expression-level effect inference

### [TODO] E2-T01：expr_surface_row + canonical semantic facts

- 参考：`PLAN.md` §4；`EFFECT_INFER.md` §129-153。
- 必须实现的内容：
  1. 每个 expr 计算 `expr_surface_row`，按统一规则 union 子表达式 + callee `published_surface_row`，按 handler 规则移除被本地处理且不 outward 的 effect。
  2. delegated property / operator / loop / computed property / constructor-init 等在 typecheck/HIR 发布统一 canonical semantic facts（core call/op），effect inference 只消费这些 facts，**删除按语法/名称的 effect 特判后门**。
- 必须遵从的约束：不为任何语法开 effect 后门；委托读写、operator、for-loop 都映射到统一 core call fact。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：所有 outward effect 来源统一可见，无语法特判残留。
- 依赖：E1-T01R
- 完成记录：（待填）

### [TODO] E2-T01R：Review E2-T01 表达式 effect 推断统一性
- 必须实现的内容：复核无后门、委托/operator/loop 经统一 facts、handler 移除规则正确。
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E2-T01
- 完成记录：（待填）

---

## E3：消费面 + dispatch ABI

### [TODO] E3-T01：调用点/override/dispatch 消费 published facts

- 参考：`PLAN.md` §3/§5；`EFFECT_INFER.md` §220-228、§258。
- 必须实现的内容：
  1. 调用点 effect 检查读 callee 的 `published_surface_row`（实例化），不再读 `fun.effects` 或重建。
  2. override/impl 一致性用 published facts；实现可更 Pure 但不得扩展 outward row、不得改 dispatch ABI。
  3. dynamic dispatch（itable/vtable）的 effect ABI 由 slot 的 published row 固定；call site 只读静态 receiver slot 契约，不 union 实现。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：调用点/override/dispatch 全部消费 facts；dispatch slot ABI 固定。
- 依赖：E2-T01R
- 完成记录：（待填）

### [TODO] E3-T01R：Review E3-T01 消费面与 dispatch ABI
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E3-T01
- 完成记录：（待填）

---

## E4：MIR/materialization instance-level facts

### [TODO] E4-T01：发布 `CallableInstanceEffectFacts` + instance 身份 + step schema 一致

- 参考：`PLAN.md` §2.3/§3.2；`EFFECT_INFER.md` §110-127。
- 必须实现的内容：
  1. 发布 `CallableInstanceEffectFacts { declared_surface_row, actual_surface_row, published_surface_row, step_effect_row }`（稳定表示）。
  2. instance 身份（callable/nominal key）按 §2.3：method instance 是否含 owner eff，由其 published/step row 是否引用 owner eff 决定（getValue 不含、共享；setValue 含、每 eff 一份）；class/itable key 仍 eff-aware。
  3. step schema 由 `step_effect_row` 的 canonical 形态决定，**跨 program（`late_lowered_program` 与 `abi_program`）一致**，消除 per-program 编号漂移。
  4. 按新表示重写 E0-T00 标注的 owner-eff "必需基础"（`is_generic`/`has_eff_param`/boundary upcast）。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：instance facts 完整；getValue/setValue 不再分叉；跨 program step schema 一致。
- 依赖：E3-T01R
- 完成记录：（待填）

### [TODO] E4-T01R：Review E4-T01 instance facts 与 step schema 一致性
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E4-T01
- 完成记录：（待填）

---

## E5：下游消费 + 删除重建

### [TODO] E5-T01：下游消费 instance facts，删除全部重建逻辑

- 参考：`PLAN.md` §8；E0-T00 清理清单。
- 必须实现的内容：
  1. effect-facts/LIR/codegen 改为消费已发布 instance facts；删除 `callable_owner_context` receiver 重建、itable `materialize_member_impl_fqn_for_owner` 重建、lir-facts `target_callable_key` mangle 重建 / `loose_instance_signature`、`dispatch_target_owner_eff_args` 等。
  2. 新增"零重建"grep 守卫测试：断言下游不再从 receiver/mangle/TypeId 重建 effect/instance 身份。
- 必须遵从的约束：删除后 owner-eff/委托 codegen 不得出现 eff-less/eff-Pure 分叉或 invoke-shell 悬空。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：下游无重建残留；守卫绿。
- 依赖：E4-T01R
- 完成记录：（待填）

### [TODO] E5-T01R：Review E5-T01 重建清零
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E5-T01
- 完成记录：（待填）

---

## E6：边界 + 递归

### [TODO] E6-T01：边界消费完整 facts + 递归 fixed-point

- 参考：`PLAN.md` §6；`EFFECT_INFER.md` §156-166、§231-239。
- 必须实现的内容：entry/export 完整 published row = `Pure!`；`@NoGC` 禁 eff-row 参数且 direct/published Pure/Pure!；`@Extern` surface row = 声明且 Pure/Pure!；递归 SCC fixed-point 推导，不收敛/不可定 → 显式-row 诊断。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 依赖：E5-T01R
- 完成记录：（待填）

### [TODO] E6-T01R：Review E6-T01 边界与递归
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E6-T01
- 完成记录：（待填）

---

## E7：Inference 放宽

### [TODO] E7-T01：concrete 省略纯传递 row + 删旧规则

- 参考：`PLAN.md` §7；`EFFECT_INFER.md` §22-70、§241-250。
- 必须实现的内容：concrete 函数尾部纯传递 row 可省略（发布推导 published row）；显式 row 必须是完整调用面契约；函数类型/higher-order/无 body API/dispatch slot 仍强制显式；删除"public 省略 effect 必报错"旧规则，替换为 direct-effect 判定。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；新增 inference fixtures。
- 依赖：E6-T01R
- 完成记录：（待填）

### [TODO] E7-T01R：Review E7-T01 inference 放宽
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E7-T01
- 完成记录：（待填）

---

## E8：全量收口

### [TODO] E8-T01：全量 golden/fixture/四后端/跨平台 + spec 回写

- 必须实现的内容：更新所有受 `EffectRowTemplate`/facts 改动影响的 golden（HIR/effect-lowered/MIR/IR 等，确认仅表示/编号变化无语义回归）；四后端（baseline moving/non-moving、immix、minimal、hosted）+ 跨平台（linux/amd64、macos/aarch64）回归；回写 `SCOOP_FULL_SPEC.md` / `docs/spec/*` 的 effect 章节与 `EFFECT_INFER.md` 落地说明。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；`python3 tools/spec_fixtures.py check`。
- 依赖：E7-T01R
- 完成记录：（待填）

### [TODO] E8-T01R：Review E8-T01 收口
- 验证：`python3 tools/run_fixtures.py`
- 依赖：E8-T01
- 完成记录：（待填）

---

## 承接：PLAN-2 的 P5 委托库化剩余目标（基于新 effect facts 完成）

> 这些任务取代 PLAN-2/TODO-2 的 P5-T02B0 / P5-T02B / P5-T03 / P5-T03R。原"手工修 owner-eff ctor/itable/cross-cone ABI handoff"的思路作废，改为在 effect 重构（E0–E8）提供的统一 facts 之上完成。

### [TODO] P5-DLG-01：在新 effect facts 之上完成 lazy/observable/vetoable 库化端到端

- 参考：`PLAN-2.md` §5 P5、`TODO-2.md` P5-T02B0/P5-T02B；`sysroot/lib/scoop.delegates/src/delegates.scoop`。
- 必须实现的内容：
  1. `ObservableProperty<V, eff E>` / `VetoableProperty<V, eff E>` / `ReadWriteProperty<.., eff E>` 同步 effect-polymorphic：`onChange` 为 `(V,V)->Unit/E` / `(V,V)->Bool/E`，`setValue` 声明 `/ E`；`observable`/`vetoable` 回调在锁外执行。
  2. `Lazy`/`lazy` initializer 收紧为 `() -> V / Pure!`（三模式 None/Publication/Synchronized 语义不变，不支持 effectful/async initializer）。
  3. 端到端通过 `tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop` 等委托回归（依赖 E2 的委托 canonical semantic facts、E3 的 dispatch slot 固定 row、E4 的 instance 身份/step schema 一致）。
- 必须遵从的约束：不恢复 lazy/observable/vetoable 专用 lowering，不按名称特判，不在 core/delegates 引入 async executor。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 依赖：E8-T01R
- 完成记录：（待填）

### [TODO] P5-DLG-01R：Review P5-DLG-01 委托库化
- 验证：`python3 tools/run_fixtures.py`
- 依赖：P5-DLG-01
- 完成记录：（待填）

### [TODO] P5-DLG-02：委托同步回归 + 零硬编码守卫扩展

- 参考：`TODO-2.md` P5-T03。
- 必须实现的内容：lazy/observable/vetoable run-pass/hir fixtures 切到库实现并验证语义不变（lazy 三模式、observable 写后回调、vetoable 否决不写、并发可见性）；"零编译器硬编码"grep 守卫扩展到三者与 `scoop.sync.Mutex` 注入点；删除/迁移被取代的旧委托合成 fixtures。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 依赖：P5-DLG-01R
- 完成记录：（待填）

### [TODO] P5-DLG-02R：Review P5-DLG-02 回归与守卫
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：P5 委托库化整体收口；属性委托对编译器透明。
- 依赖：P5-DLG-02
- 完成记录：（待填）
