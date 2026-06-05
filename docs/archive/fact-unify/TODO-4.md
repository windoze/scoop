# TODO-4：effect 语义收口 + owner-eff 委托端到端（批 4）

> 计划基线：[`PLAN.md`](./PLAN.md) §4 批 4、§5/§6/§7；依据 `EFFECT_INFER.md` §5/§6/§7/§156-166/§220-250；承接 `docs/archive/TODO-2.md` 的 P5-T02B0/P5-T02B/P5-T03。
> 索引入口：[`TODO.md`](./TODO.md)
> 目标：在前三批提供的稳定 identity + 完整 facts + 纯消费基础上，收口 effect 语义（分层 row 契约、dispatch effect-ABI 固定、边界、递归、inference 放宽），完成 `lazy`/`observable`/`vetoable` 库化端到端，恢复批 1 bypass 的 fixture/test，全量回归 + spec 回写。
> 依赖：批 3（`TODO-3.md`）全部完成。

## 任务索引

| 任务 | 状态 | 目标 |
| --- | --- | --- |
| T4-01 | [TODO] | 分层 row 契约 + dispatch effect-ABI 固定 + 边界（entry/export/`@NoGC`/`@Extern`）+ 递归 fixed-point |
| T4-01R | [TODO] | Review T4-01 |
| T4-02 | [TODO] | Inference 放宽（concrete 省略纯传递 row）+ 删旧 public-omitted 必报错规则 |
| T4-02R | [TODO] | Review T4-02 |
| T4-03 | [TODO] | owner-`eff` 委托库化端到端（lazy/observable/vetoable，承接 P5-T02B0/P5-T02B） |
| T4-03R | [TODO] | Review T4-03 |
| T4-04 | [TODO] | 恢复 bypass 的 fixture/test + 委托守卫扩展 + 全量回归 + spec 回写 |
| T4-04R | [TODO] | Review T4-04 |

---

### [TODO] T4-01：分层 row 契约 + dispatch effect-ABI 固定 + 边界 + 递归

- 参考：`EFFECT_INFER.md` §22-70/§156-166/§220-239；`PLAN.md` §5/§6。
- 必须实现的内容：
  1. 落实分层 row 语义规则：显式 `/ Row` = 完整调用面契约且校验 `inferred ⊆ declared`；显式 row 不能只列 direct 增量；调用点/override 用 published facts。
  2. dynamic dispatch effect-ABI 固定：interface/abstract/open/default method 的 published row 固定 itable/vtable slot ABI，override 不得扩展 outward row、不改 ABI；dispatch call site 只读静态 receiver slot 契约。
  3. 边界：entry/export 完整 published row = `Pure!`；`@NoGC` 禁 eff-row 参数且 direct/published Pure/Pure!；`@Extern` surface row = 声明且 Pure/Pure!。
  4. 递归 SCC fixed-point 推导 inferred row；不收敛/effect-polymorphic recursion/未解析分派/跨 cone body 缺失 → 要求显式 outward row（清晰诊断）。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；新增 typecheck/effect fixtures。
- 依赖：T3-04R
- 完成记录：（待填）

### [TODO] T4-01R：Review T4-01
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T4-01
- 完成记录：（待填）

### [TODO] T4-02：Inference 放宽 + 删旧规则

- 参考：`EFFECT_INFER.md` §66-70/§241-250。
- 必须实现的内容：concrete 函数尾部纯传递 row 可省略（发布推导 published row）；函数类型/higher-order 参数/返回/字段/property/typealias、带 eff-row 参数的函数类型、无 body API、所有 dispatch slot method 仍强制显式完整 row；删除"public concrete 省略 effect 必报错"旧规则，替换为 direct-effect 判定（direct 非 Pure 才要求显式）。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；新增 inference fixtures。
- 依赖：T4-01R
- 完成记录：（待填）

### [TODO] T4-02R：Review T4-02
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T4-02
- 完成记录：（待填）

### [TODO] T4-03：owner-`eff` 委托库化端到端

- 参考：`docs/archive/TODO-2.md` P5-T02B0/P5-T02B；`sysroot/lib/scoop.delegates/src/delegates.scoop`。
- 必须实现的内容：
  1. `ObservableProperty<V, eff E>`/`VetoableProperty<V, eff E>`/`ReadWriteProperty<.., eff E>` 同步 effect-polymorphic：`onChange` 为 `(V,V)->Unit/E`/`(V,V)->Bool/E`，`setValue` 声明 `/ E`，回调锁外执行；`Lazy`/`lazy` initializer 收紧为 `() -> V / Pure!`（三模式语义不变）。
  2. 凭借批 1–3 的稳定 identity + instance facts + dispatch slot 固定 row，owner-eff 委托端到端工作（取消 bypass 的 `delegated_property_observable_raise_does_not_poison_mutex.scoop` 等通过），无 eff-less/eff-Pure 分叉、无 unpublished dispatch target、无 invoke-shell 悬空。
- 必须遵从的约束：不恢复 lazy/observable/vetoable 专用 lowering、不按名称特判、不在 core/delegates 引入 async executor。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
- 依赖：T4-02R
- 完成记录：（待填）

### [TODO] T4-03R：Review T4-03
- 验证：`python3 tools/run_fixtures.py`
- 依赖：T4-03
- 完成记录：（待填）

### [TODO] T4-04：恢复 bypass + 守卫扩展 + 全量回归 + spec

- 必须实现的内容：
  1. 移除 T1-00 给 delegate fixture/owner-eff test 加的 `IGNORE-UNTIL-FIX`/`#[ignore]`，确认全部通过（不得遗留永久跳过）。
  2. lazy/observable/vetoable run-pass/hir fixtures 切到库实现并验证语义不变；"零编译器硬编码"grep 守卫扩展到三者与 `scoop.sync.Mutex` 注入点；删除/迁移被取代的旧委托合成 fixtures。
  3. 更新所有受 `EffectRowTemplate`/stable key/facts 改动影响的 golden（确认仅表示/编号变化无语义回归）；四后端 + 跨平台回归；回写 `SCOOP_FULL_SPEC.md`/`docs/spec/*` 的 effect 章节与 `EFFECT_INFER.md` 落地说明。
- 验证：`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`；`python3 tools/spec_fixtures.py check`。
- 完成条件：bypass 全部恢复且绿；委托对编译器透明；全量/四后端/双平台绿；spec 同步。
- 依赖：T4-03R
- 完成记录：（待填）

### [TODO] T4-04R：Review T4-04
- 验证：`python3 tools/run_fixtures.py`
- 完成条件：Fact 体系重构整体收口。
- 依赖：T4-04
- 完成记录：（待填）
