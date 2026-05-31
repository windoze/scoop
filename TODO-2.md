# TODO-2：完整 fact 发布 + self-contained artifact（批 2）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 2、§3；依据 `FACT_GAPS.md` FG-06/08/09(发布)/10/11(必发)/12/13/15、`EFFECT_INFER.md` §3/§4。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：HIR/MIR/P4 完整发布分层 effect facts + site/event/provenance/source-signature/boundary facts；artifact 自包含，下游不再回看 `LoweredHir`/`MaterializedMir` side table。
> 依赖：批 1（`TODO-1.md`）全部完成。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T2-01 | [TODO] | HIR 分层 `CallableSourceEffectFacts` + 统一 expression inference + canonical semantic facts（含 hidden init） |
| T2-01R | [TODO] | Review T2-01 |
| T2-02 | [TODO] | MIR `CallableInstanceEffectFacts` + effect-event/site-inventory/provenance facts + backend contracts 收口 |
| T2-02R | [TODO] | Review T2-02 |
| T2-03 | [TODO] | P4 纯消费上游 facts 产出 instance effect facts（local control 必发、call-site target/surface） |
| T2-03R | [TODO] | Review T2-03 |

---

### [TODO] T2-01：HIR source-level effect facts + 统一 expression inference

- 参考：`PLAN.md` §2.3/§3/§4；`EFFECT_INFER.md` §72-153；`FACT_GAPS.md` FG-06/09(source)。`crates/scoopc_hir/src/typecheck/expr/stmt.rs` `check_required_effects_for_fun_decl`、`stage.rs` `FunctionEffectContract`。
- 必须实现的内容：
  1. 发布 `CallableSourceEffectFacts { declared_surface_row, direct_effect_row, inferred_surface_row_template, published_surface_row_template, row_is_closed, inference_status }`（`EffectRowTemplate`）；`check_required_effects_for_fun_decl` 从"只报错"改为"发布 facts"。
  2. 统一 expression-level effect inference：每个 expr 算 `expr_surface_row`（union 子表达式 + callee published row，按 handler 规则移除本地处理不 outward 的 effect）。
  3. canonical semantic expansion facts：delegated property / operator / loop / computed property / constructor-init 发布统一 core call/op fact，effect inference 只消费它们，**删除按语法/名称的 effect 后门**。
  4. **FG-06**：发布 `HiddenInitializerEffectFact`（class ctor / object init / top-level init 的 hidden-effect summary），替代 MIR lowering 里 `HiddenInitEffectAnalyzer` 的重扫（搬运留 T2-02）。
  5. interface(含 default)/abstract/open method 的 `published_surface_row_template` 必须来自显式契约。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；新增/更新 typecheck fixtures。
- 完成条件：HIR 发布完整分层 source facts + canonical semantic facts；表达式 effect 无语法后门；hidden init effect 由 HIR fact 提供。
- 依赖：T1-02R
- 完成记录：（待填）

### [TODO] T2-01R：Review T2-01
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-01
- 完成记录：（待填）

### [TODO] T2-02：MIR instance facts + effect-event/provenance + backend contracts 收口

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-08/10/12/13。
- 必须实现的内容：
  1. 发布 `CallableInstanceEffectFacts { declared_surface_row, actual_surface_row, published_surface_row, step_effect_row }`（稳定表示）；method instance 身份按 published/step row 是否引用 owner eff 区分（getValue 共享 eff-less；setValue eff-keyed；class/itable key eff-aware）。
  2. **FG-08**：发布 `MirEffectEventFact` / `MirBlockEffectRegionFact` / `MirSiteInventoryFact`（结构化 effect event stream、block-to-site inventory、handled-region/cleanup/suspend boundary），供 P4 solver 消费，替代 P4 扫 MIR shape。
  3. **FG-10**：发布 `CallableValueProvenanceFact` / `ResultProvenanceFact`（函数值 points-to/provenance + pass-rewritten summary 稳定查询面）。
  4. **FG-12**：boundary discovery/segmentation 发布结构化 `BoundarySourceContract`（boundary statement anchor、result local、carrier operand source、arg source、closure env decomposition），供 P5 消费。
  5. **FG-13**：MIR facts family/metadata 补 `eff_args`/layout/vtable/itable/extern/native/global init contract，把 `MaterializedBackendContracts` 收口为 fact artifact；搬运 T2-01 的 `HiddenInitializerEffectFact` 到 MIR site metadata。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：MIR facts 自包含；instance 身份不再 eff 分叉；effect-event/provenance/boundary/backend contract 由 fact 提供。
- 依赖：T2-01R
- 完成记录：（待填）

### [TODO] T2-02R：Review T2-02
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-02
- 完成记录：（待填）

### [TODO] T2-03：P4 纯消费上游 facts 产出 instance effect facts

- 参考：`PLAN.md` §3；`FACT_GAPS.md` FG-08/09(发布)/11(必发)。
- 必须实现的内容：
  1. P4 solver 消费 `MirEffectEventFact`/site/region facts，不再扫 materialized MIR statement/terminator shape（`effect_facts/builder.rs` `scan_block_sites`/`scan_block_*`）。
  2. call-site target/declared row 用已发布 `CallSiteTargetFact`/`CallSiteSurfaceEffectFact`（FG-09），删除 `build_direct_like_call_site`/`union_candidate_rows` 的 overload 选择与 declared row 重算。
  3. **FG-11**：`BodyEffectFacts.local_control_step_schema` 设为 P4 必发 contract（owner step schema 由 `step_effect_row` 确定）。
  4. callable value/closure effect 用 T2-02 的 provenance fact（FG-10），删除 P4 局部数据流恢复。
- 必须遵从的约束：P4 不再做 overload/数据流/effect 重建；env/dispatch table 重建（FG-07）留批 3，本任务先把 effect 求解改为消费 facts。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 完成条件：P4 effect 求解纯消费上游 facts；local control schema 必发。
- 依赖：T2-02R
- 完成记录：（待填）

### [TODO] T2-03R：Review T2-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T2-03
- 完成记录：（待填）
