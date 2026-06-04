# TODO — P2：LIR 内部重设计（自包含 `LirProgram`）

> 计划：[`PLAN.md`](./PLAN.md)；设计：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md) §13/§2.7/§14。
> 三个子阶段按序：**P2a 身份地基 → P2b 折叠 facts → P2c lift 指令/消 overlay**。每任务后跟一个 review；每任务收尾跑 §9 基线。

## 0. 非目标 / 范围纪律

- 不动 P3+（effect-facts/MIR/HIR）；P2 只重塑 LIR 内部表示与其消费（codegen）。
- P2a 不改 facts 结构、不 lift 指令；P2b 不 lift 指令；三者解耦推进，各自全套 fixture 绿后再进下一子阶段。
- 不新增任何字符串 live key / 缺-fact fallback（违反即 review 打回）。

## 1. 代码地图（按符号定位，勿全库搜索）

**handoff/容器**：
- `crates/scoopc/src/pipeline/lir_artifact.rs`：`LirArtifact { cone, program: LateLoweredProgram, facts: LirFacts /*P2b 删*/, base_context, mir: Option<MaterializedMir> /*P2c 删*/, object_files }`、`CodegenInput`。
- `crates/scoopc_lir/src/effect_lowered/ir.rs`：`pub struct LateLoweredProgram`（字段：`step_types`/`resume_packings`/`continuation_objects`/`surface_resume_dispatch_inventory`/`callables: Vec<LateLoweredCallable>`/`class_ctor_init_bodies: HashMap<String,_>`/`source_class_ctor_calls`/`stable_instance_keys`/`dump_*`）；`type LateLoweredSourceBody = crate::mir::Body`(:343)；`struct LateLoweredStateSlice { block_id, start_statement_index, end_statement_index, includes_terminator }`(:3591)；`LateLoweredCallable.lir_callable_key: Option<StableLirCallableKey>`(:1184)。

**身份（P2a）**：
- `crates/scoopc_ids/src/lib.rs:260`：`struct StableLirCallableKey { canonical_text: String, readable_path: String }`（+ `StableCanonicalKey`/`StableSymbolKey` impl）。
- 生产代码 139 处使用；集中在 `crates/scoopc_lir_facts/src/contract.rs`（~30 处：`owner_callable`/`callable`/`target_callable_key`/`candidate_targets`/`method_impl_targets`/`impl_member_target`，及 `callable_symbols: BTreeMap<StableLirCallableKey,_>`(:619)、`closure_identity: BTreeMap<StableLirCallableKey,_>`(:623)）；`lib.rs:38/60` `callables: BTreeMap<StableLirCallableKey, LirCallableFacts>`；`ir.rs:1184/1310/1369`。

**facts（P2b）** — `crates/scoopc_lir_facts/src/lib.rs:24` `struct LirFacts` 18 组。Fold map：
| 组 | 现 key | 归属节点 |
|---|---|---|
| `callables` | StableLirCallableKey | **callable 节点**（主锚） |
| `source_call_sites`/`class_ctor_call_sites`/`reflection_call_sites` | (owner_callable, site_id) | callable 体内 **call-site 节点** |
| `dynamic_invokes`/`dispatches` | (owner_callable, site_id) | callable 体内 **invoke/dispatch 节点** |
| `global_init` | LirGlobalRootKey(FQN) | **global root 节点** |
| `physical_layout`(classes/enums/abi_symbols/vtables/itables) | String | **nominal 节点** |
| `source_signatures`/`intrinsic_callables` | String(FQN) | **callable 节点**（key→句柄） |
| `class_ctor_inits` | String | class ctor init 节点 |
| `continuation_objects` | id | effect-step callable 的 control body |
| `summary`/`opt_pipeline`/`type_context`/`step_types`/`resume_packings`/`surface_resume_dispatches` | — | **保留 program 级字段**（程序全局） |
- 消费点（改 walk）：`call/abi.rs:339/352/395`、`effect_lowered/layout/lookup.rs:128/146`、`effect_lowered/layout/dynamic_invoke.rs:16`、`pipeline/llvm_codegen_stage` emit、`mod.rs:927`。

**MIR 指令集（P2c lift 来源）** — `crates/scoopc_mir/src/mir/mod.rs`：`Body`(:2057, `locals`/`blocks`/`start`)、`BasicBlock`(`stmts`/`terminator`/`is_cleanup`)、`StatementKind`(:2424, `Nop`/`Assign`/`StoreMember`/`StoreTopLevelVar`/`Todo`)、`Rvalue`(:2909, 24 变体)、`TerminatorKind`(:3060, `Return`/`ResumeUnwind`/`Goto`/`CondBr`/`Unreachable`/`Perform`/`Handle`/`Todo`)、`Operand`(:2468, `Local`/`Const`)、`ConstValue`、`CallKind`(:2672, `Direct`/`Closure`/`FunValue`/`FunPtr`/`Virtual`/`Interface`/`Resume`)、`MemberTarget`/`DispatchMetadata`/`TopLevelRef`/`PerformMetadata`、transport（`mir/transport.rs`）。**字符串 FQN 引用 11 处**（`CallKind::Direct.callee_fqn`、`Closure.fn_ptr`、`Perform.op_fqn`、`TopLevelRef.fqn`、`StoreTopLevelVar.fqn`、`DispatchMetadata.owner_fqn/member_fqn`、`MemberTarget::*.fqn`、`ClassCtor.class_fqn`、`PerformResult.op_fqn`、GC intrinsic）→ 全转句柄。**注：MIR 的 `StatementKind::Todo`/`Rvalue::Todo`/`Rvalue::UnresolvedName`/`TerminatorKind::Todo` 等占位/逃逸变体不进 LIR（LIR total，见 T2-07）。**
- codegen MIR-body walker（P2c 改走 LIR 指令）：`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs`（stmt :318、rvalue :363、terminator :425、callkind :347）。

## 2. 身份类型（P2a 目标）

```rust
// = program.callables 的下标；artifact 内 live 引用一律用它（§2.7）
pub struct LirCallableId(u32);
// 跨 cone / 序列化 / map key 的紧凑身份（从 StableLirCallableKey.canonical_text 派生）
pub struct LirCallableHash(/* 定长 hash */);
// StableLirCallableKey 保留为「稳定身份来源」，readable_path 仅调试
```

---

## 3. P2a — 身份地基

### [DONE] T2-01：引入 `LirCallableId` / `LirCallableHash`
- 在 `scoopc_ids`（或 lir crate）定义 `LirCallableId(u32)`、`LirCallableHash`（从 `StableLirCallableKey::canonical_text` 派生，定长）。`StableLirCallableKey` 保留；`readable_path` 标注「仅调试」。
- 在 LIR 阶段边界建一次 `HashMap<&StableLirCallableKey, LirCallableId>`（= `program.callables` 索引）与反查；这是**唯一**「按 stable key 解析」的可失败点。
- 验收：编译通过；建立映射的单测。

完成记录（2026-06-04）：
- `scoopc_ids` 新增 `LirCallableId` 与定长 128-bit `LirCallableHash`，hash 从 `StableLirCallableKey::canonical_text` 派生；`readable_path` 文档明确为诊断/符号标签用途。
- `scoopc_lir` 新增 `LirCallableIndex`，按 `LateLoweredProgram.callables` 顺序建立 stable key → `LirCallableId`、id → key、id → hash 的边界索引；缺失 key、重复 key、未知 key/id 都返回显式错误。
- `LirArtifact::new` 在 LLVM LIR handoff 边界构建 callable 索引；入口解析先由 stable key 解析为 `LirCallableId`，再按 id 访问 `program.callables`。
- 新增单测覆盖 id/hash 稳定性、key 命中、key 未命中、id 未命中与重复 key 错误。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-01-R：Review T2-01
- 关注点：`LirCallableId` 语义 = `program.callables` 下标（与 arena 一致，无第二套编号）；`LirCallableHash` 派生确定、稳定、可序列化；map 构建是唯一 fallible 解析点。
- 确认：`cargo build`/`clippy -D warnings`；单测覆盖「key→id 命中/未命中（未命中=错误而非 panic）」。

完成记录（2026-06-04）：
- Review 发现 `StableLirCallableKey` 虽已标注 `readable_path` 仅用于诊断，但派生的 `Eq`/`Hash`/`Ord` 仍把 `readable_path` 纳入身份；这会让相同 canonical text、不同 debug path 的 callable key 逃过重复检测或在 live lookup 中 miss。
- 修正 `StableLirCallableKey` 的 `PartialEq`/`Eq`/`Hash`/`PartialOrd`/`Ord` 为 canonical-text-only，保持 `readable_path` 仅用于诊断和符号标签。
- 补充单测覆盖 stable key canonical-only identity、hash/order 集合去重、`LirCallableIndex` 用不同 debug path 命中同一 `LirCallableId`、以及同 canonical 不同 debug path 的重复 key 错误。
- Review 确认 `LirCallableId` 仍按 `LateLoweredProgram.callables` 下标寻址，`LirCallableHash` 从 canonical text 派生且为固定 128-bit 可序列化句柄，`LirCallableIndex` 在 LIR handoff 边界构建并对缺 key、重复 key、未知 key/id 返回显式错误。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo test -p scoopc pipeline::llvm_codegen_stage::tests::llvm_function_abi_entry_shells_use_direct_entry -- --exact`；`cargo test -p scoopc pipeline::llvm_codegen_stage::tests::llvm_value_boxing_transport -- --exact`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-02：`callables` 容器改为按 `LirCallableId` 寻址
- `LateLoweredProgram.callables: Vec<LateLoweredCallable>` 已是 Vec → 直接以下标为 `LirCallableId`。把 `LirFacts.callables: BTreeMap<StableLirCallableKey, LirCallableFacts>`（`lib.rs:38/60`）与 `callable_symbols`/`closure_identity`（`contract.rs:619/623`）的查找改为经 `LirCallableId`（过渡期保留 stable key 字段，仅切换 live 访问路径）。
- 验收：codegen/消费侧不再用 `StableLirCallableKey` 做 callable 查找。

完成记录（2026-06-04）：
- `LirFacts.callables`、`LirPhysicalLayoutFacts.callable_symbols`、`closure_identities` 改为 `BTreeMap<LirCallableId, _>`；payload 中的 stable key / body-version owner 保留给过渡期调试与 T2-03 跨引用迁移。
- `lir_facts_builder` 在 LIR 边界按 `LateLoweredProgram.callables` 下标建立 id map，并用 id 发布 callable facts、callable symbol facts 和 closure identity facts。
- LIR facts verifier/dump、LLVM 入口解析、ABI layout callable facts lookup、closure identity lookup、reachability 与相关测试改为 id 键访问；`crates/scoopc_codegen_llvm` 中 `callables.get(` 仅剩 `get(&id)`，`callable_symbols.get(` 零命中。
- 更新 10 个 `effect_lowered` golden，反映 callable dump 中的 `LirCallableId` 和 id 顺序。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-02-R：Review T2-02
- 关注点：所有 callable 查找走 `LirCallableId`；map 仅在边界构建处出现；无新增 string 查找。
- 确认：`grep -rn "callables.*get(.*StableLirCallableKey\|by.*fqn" crates/scoopc_codegen_llvm` 在 callable 查找路径零命中；全套基线绿。

完成记录（2026-06-04）：
- Review 发现 `EntryRef` 仍携带 `StableLirCallableKey`，LLVM emit/main wrapper 通过 `callable_by_lir_key` 做入口 body lookup；已改为在 LIR 边界保存 `LirCallableId`，后续入口路径全部用 `callable_by_id`。
- Review 发现 codegen 与 LIR facts builder 仍有按 root FQN 扫描 `callables` / `callable_symbols` 的 T2-02 相关查找；已改为先解析 `LirCallableId` 再对 id-keyed map `.get(&id)`。
- 确认 `crates/scoopc_codegen_llvm` 中 `callables.get(...StableLirCallableKey)`、`callable_by_lir_key`、`callable_symbols.values()/iter()` lookup 路径零命中；剩余 `StableLirCallableKey` 使用为 T2-03/T2-05 仍要迁移的跨引用/site owner 过渡字段或测试构造。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-03：迁移跨引用字段到 `LirCallableId`
- `contract.rs` 的 `owner_callable`/`callable`/`target_callable_key`/`candidate_targets`/`method_impl_targets`/`impl_member_target`（约 30 处）改为 `LirCallableId`（跨 cone 引用用 `LirCallableHash`，经 `LirArtifact.deps` 解析为本地 `LirCallableId`）。`ir.rs:1184` `lir_callable_key` 同步。
- 验收：139 处 `StableLirCallableKey` 使用降到「仅稳定身份来源 + 边界 map + 调试」。

完成记录（2026-06-04）：
- `scoopc_lir_facts::contract` 中的 live callable owner/target/payload 字段迁移为本 cone `LirCallableId`；跨 cone / bodyless declaration 目标使用新增 `LirCallableRef::ExternalHash(LirCallableHash)` 表示。
- `LirCallSiteContract`、dynamic invoke、dispatch、class ctor/reflection/source call-site key、vtable/itable target、ABI symbol、closure identity 与 callable symbol facts 全部改为 id/hash 句柄，不再携带 `StableLirCallableKey` 作为 live 关联字段。
- `lir_facts_builder` 在构造边界把 stable key 解析为本地 id 或 hash 引用；`LirCallableIndex` 增加 hash→id 解析，`LirArtifact` 暴露 hash 解析入口。
- LLVM reachability、ABI/layout lookup、dispatch、closure identity、source/class-ctor call-site lookup 改为 id/hash 路径；修复 ABI visibility program 与 primary facts id 顺序不同导致的 current callable id 错配。
- 更新 effect-lowered goldens，反映 callable symbol / source-site dump 中的 id-based 引用与 id 顺序。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-03-R：Review T2-03
- 关注点：跨 cone 引用用 hash、本 cone 用 id，无混用；`candidate_targets`/`method_impl_targets` 等列表全句柄；无悬空（构造时即解析）。
- 确认：`grep -c "StableLirCallableKey" crates/`（生产）显著下降且剩余均为身份来源/边界/调试；全套基线绿。

完成记录（2026-06-04）：
- Review 确认 `LirCallSiteContract`、dynamic invoke、dispatch、vtable/itable target、source/class-ctor/reflection call-site key 等 live owner/target 字段使用 `LirCallableId` 或 `LirCallableRef::{Local, ExternalHash}`；`contract.rs` 不再携带 `StableLirCallableKey` 作为 live 关联字段。
- Review 发现 codegen 仍保留未使用的 `current_lir_callable_key` 状态和 stable-key root lookup helper；已删除该状态、相关赋值，以及未使用的 `LateLoweredProgram::callable_by_lir_key`，当前 codegen callable 上下文仅保留 `LirCallableId` 路径。
- Review 发现 verifier/reachability 对缺失的 `LirCallableRef::Local` 仍可落入 declaration ABI 兜底；已改为本地 id 必须解析到 callable inventory，只有 `ExternalHash` 可走 declaration ABI 解析，并补充 dangling local call-target / ABI-symbol 回归测试。
- 确认 `current_lir_callable_key`、`lir_callable_key_for_root`、`callable_by_lir_key` 在 `crates/` 零命中；`crates/scoopc_codegen_llvm/src/llvm/codegen` 中 `StableLirCallableKey` 零命中；`rg "StableLirCallableKey" crates --glob "*.rs" | wc -l` 为 90（含测试/身份来源/边界/调试）。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

---

## 4. P2b — 折叠 `LirFacts` 进节点

### [DONE] T2-04：per-callable fact 挂到 callable 节点
- 把 `callables`/`source_signatures`/`intrinsic_callables` 的 value 内容并入 `LateLoweredCallable`（或其旁挂结构）。string key（FQN）→ `LirCallableId`。
- 验收：消费侧从「`source_signatures.get(fqn)`」（`call/abi.rs:395`）变为 callable 节点字段访问。

完成记录（2026-06-04）：
- `LateLoweredCallable` 新增已发布 per-callable facts、source signatures、intrinsic callable metadata payload；`LateLoweredProgram` 新增 declaration sidecar 与 source/intrinsic/callable-id 查询方法，构建 facts 后按 `LirCallableId` 回填到 callable 节点。
- `resolve_entry_ref`、LLVM signature/intrinsic lookup、ABI layout callable facts lookup、reachability 与 HIR/direct-call root resolution 改为从 active LIR program/callable 节点读取，不再消费 `source_signatures.get(fqn)` / `intrinsic_callables.get(fqn)` / `published_lir_facts.callables` 路径。
- 修复迁移暴露的 bincode 非自描述序列化问题：新增 LIR program 字段使用 `serde(default)` 但不使用 `skip_serializing_if`；`WIRE_SCHEMA_VERSION` 升至 1.13。
- 补充回归测试覆盖 callable 节点持有 callable facts/source signatures，以及 program 持有 intrinsic callable metadata。
- 修复 active ABI visibility codegen 上下文：`MainCodegen` 同步切换 active LIR program 与 active LIR facts，避免 source-site/signature/intrinsic 查询混用不同 artifact。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc effect_lowered_program -- --nocapture`；`cargo test -p scoopc frontend::tests::dependency_frontend_cache_hit_uses_artifact_without_reading_source -- --exact --nocapture`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-04-R：Review T2-04
- 关注点：facts 真正成为节点拥有的数据（非旁表 join）；FQN key 全转句柄；无缺-fact fallback 复活。
- 确认：`call/abi.rs` 无 `source_signatures`/`intrinsic_callables` map 查找；基线绿。

完成记录（2026-06-04）：
- Review 确认 `call/abi.rs` 不再通过 `source_signatures` / `intrinsic_callables` 平表 map 查找；source/intrinsic metadata 读取走 active LIR program 的 callable/declaration 节点。
- Review 发现 declaration-only `source_signature` 虽已从 LIR program 节点读取，但 `published_callable_signature_with_names_impl` 仍把该签名标成主 codegen `TypeStore` 所有，ABI visibility/source TypeStore 与主 cone 不一致时会绕过跨 TypeStore 映射。
- 修正 declaration source signature 分支，返回 `published_late_lowered_types()` 作为签名 owner，防止非 codegen `TypeId` 被直接当作 codegen id 使用。
- 新增 standalone LLVM codegen 单测 `declaration_source_signature_uses_published_typestore_owner`，覆盖 declaration-only source signature 携带 distinct published TypeStore 的映射边界。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc_codegen_llvm declaration_source_signature_uses_published_typestore_owner`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-05：per-call-site / dispatch fact 挂到体内节点
- `source_call_sites`/`class_ctor_call_sites`/`reflection_call_sites`/`dynamic_invokes`/`dispatches`（key=(owner_callable, site_id)）改为挂在对应 callable 体内的 site 节点上。
- 验收：`effect_lowered/layout/lookup.rs:128`、`dynamic_invoke.rs:16` 等改为 walk 节点，不再 `(owner,site)`→get。

完成记录（2026-06-04）：
- `LirFacts` / `LirFactGroups` 删除 `source_call_sites`、`class_ctor_call_sites`、`reflection_call_sites`、`dynamic_invokes`、`dispatches` 顶层平表；相关 payload 改挂到 `LirCallableFacts`、plain call-site、control boundary 与 control source-statement 节点。
- dynamic-invoke carrier 直接内嵌 dispatch contract；LLVM ABI materializer、main codegen context 与 reachability 改从 active LIR program/callable 节点 walk，不再通过 `(owner_callable, site_id)` 查顶层 facts map。
- `scoopc_lir_facts` 公共 API 删除这些复合 key 类型；builder 内部仅保留私有 `BuildCallSiteKey` 作为构造期去重/分组 key，并避免同一 dynamic invoke 在 plain site、boundary、source-statement 间重复发布；修复 `effect_multi_escape_indirect_callee_suspend_matrix` 暴露的重复 dynamic layout 问题。
- verifier/dump 与单测构造同步改为节点内 payload；`WIRE_SCHEMA_VERSION` 升至 1.14。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-05-R：Review T2-05
- 关注点：site 数据归 site 节点所有；`(owner_callable, site_id)` 复合 key 消失；空候选/缺 contract 由结构保证不可表示。
- 确认：相关 `.get(key)` 站点清零；基线绿。

完成记录（2026-06-04）：
- Review 确认 `LirFacts` / `LirFactGroups` 顶层 `source_call_sites`、`class_ctor_call_sites`、`reflection_call_sites`、`dynamic_invokes`、`dispatches` 平表已删除；公开旧复合 key 类型未保留，消费侧改为从 callable/site/control 节点 walk。
- Review 发现 verifier 仍允许 plain dynamic/dispatch site 缺失节点内 contract、dispatch 空候选、dynamic/dispatch owner/source 漂移，以及 boundary 分支重复挂载已发布 payload；已补齐结构校验并修正 boundary 去重。
- 新增 verifier 单测覆盖缺 dynamic-invoke contract、缺 dispatch contract、dispatch 空候选等非法 LIR facts 形状，防止缺 contract / 空候选退回为可表示状态。
- 确认剩余 `(StepSchemaId, SiteId)` / `AbiProgramOrigin` `.get(...)` 为 LLVM ABI layout 内部 cache，不是旧 `LirFacts` 顶层 `(owner_callable, site_id)` fact lookup。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-06：layout / global-init fact 归位 + 删 `LirArtifact.facts`
- `physical_layout`（classes/enums/vtables/itables/abi_symbols）挂到 nominal 节点；`global_init` 挂到 global root 节点；`class_ctor_inits` 同理。
- `summary`/`opt_pipeline`/`type_context`/`step_types`/`resume_packings`/`surface_resume_dispatches` 留作 `program` 级字段。
- 删除 `LirArtifact.facts`、`LirFacts` 顶层平表容器（其内容已分散归位）。
- 验收：`LirArtifact` 无 `facts` 字段；codegen 不再消费 `LirFacts`。

完成记录（2026-06-04）：
- `LateLoweredProgram` 新增 program-owned summary/opt-pipeline/type-context/global-init/physical-layout payload，并把 step/resume/continuation/surface-resume fact 快照作为 program 内兼容数据；`class_ctor_inits` 挂回 `LateLoweredClassCtorInitBody` 节点。
- `LirArtifact` 删除 `facts` 字段；LLVM stage output、cached dependency handoff、artifact handoff 与 `StageEmitInput` 改为只携带 LIR program + TypeStore/base context。
- LLVM codegen/reachability/ABI materializer 改从 active `LateLoweredProgram` 读取 global init、physical layout、callable symbols、ABI symbols、closure identity、class vtable/itable 与 type context；`crates/scoopc_codegen_llvm/src` 中 `LirFacts` 类型零命中。
- Cone artifact manifest/schema 与持久化布局删除 `lir_facts.bin`；artifact cache handoff 依赖 program 内 payload，不再反序列化独立 LIR facts。
- `LirStageOutput::lir_facts()` 保留为 dump/测试兼容快照生成器，不再作为阶段 handoff 独立平表容器保存。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] T2-06-R：Review T2-06
- 关注点：layout 按 nominal 句柄索引（不再 String map）；程序级组判定正确（确实全局才留 program 字段）；`LirFacts` 彻底退场或仅剩序列化外壳。
- 确认：`grep -rn "LirFacts" crates/scoopc_codegen_llvm` 零（或仅 dep 反序列化）；基线绿（含 dependency_gate）。

完成记录（2026-06-04）：
- Review 确认 `LirArtifact` 无 `facts` 字段，`crates/scoopc_codegen_llvm` 中 `LirFacts` 零命中；codegen/LLVM reachability 继续从 active `LateLoweredProgram` 读取 program-owned global/layout/type-context payload。
- Review 发现 `LirPhysicalLayoutFacts` 的 layout/ABI/layout-name 公开 map key 仍是裸 `String`，与 T2-06-R 的 nominal 句柄索引要求不一致；已新增 `LirNominalLayoutKey`、`LirAbiSymbolKey`、`LirLayoutNameKey` 并把 classes/enums/interfaces/vtables/itables/abi_symbols/layout_names 改为显式句柄 key。
- 同步修正 LIR facts builder、verifier、dump、LLVM layout lookup/reachability/test 构造路径；保持 payload 中 FQN/符号字符串只作诊断、ABI 名称或来源标签，不再作为 physical layout live map key。
- 确认 `crates/scoopc_lir_facts/src/contract.rs` 中 `LirPhysicalLayoutFacts` 的上述 map 不再使用 `BTreeMap<String, ...>`；`crates/scoopc_codegen_llvm` 中旧 `LirFacts` 消费零命中。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

---

## 5. P2c — lift 指令、消除 overlay

### [DONE] T2-07：定义 LIR 自有指令集（total，无占位变体）
- 按 §1 的 MIR 清单定义 LIR 指令：statement（assign/store-member/store-global）、rvalue（MIR 24 变体中**已实现的真实构造**的 LIR 对应，引用全句柄化：callee→`LirCallableId`、global→句柄、member/dispatch→句柄、type→`TypeId`）、terminator（含 effect 的 Perform/Handle/Resume 控制，复用现 `LateLoweredStateTerminator` 体系）、operand（local/const）。
- 句柄化 §1 列出的 11 处字符串 FQN。
- **LIR 是 total 的，禁止任何占位/逃逸变体**：明确排除 MIR 的 `StatementKind::Todo`、`Rvalue::Todo`、`Rvalue::UnresolvedName`、`TerminatorKind::Todo`（及 transport 内同类占位）。「未实现 / 未解析」**不是可表示的 LIR 状态**——它们是 MIR 的 WIP 逃逸口，正是本重构要消灭的「可表示的非法/未完成状态」。
- 验收：类型定义编译通过；与现 `LateLoweredState`/`StateTerminator` 体系衔接清楚；**LIR 指令枚举中 `grep` 无 `Todo`/`UnresolvedName`**。

完成记录（2026-06-04）：
- 新增 `crates/scoopc_lir/src/effect_lowered/instruction.rs`，定义 LIR operand、statement、rvalue、call kind、runtime metadata、transport wrapper、unwind action 与 `LirInstruction`，并在 `effect_lowered::mod` 统一导出。
- LIR rvalue 覆盖 MIR 已实现的真实构造；MIR 占位/逃逸构造未进入 LIR 指令枚举。`LirTerminator` 复用现有 `LateLoweredStateTerminator`，`LirStateBody` 显式绑定 state-owned statement 序列与 state graph terminator，供 T2-08 接入实际 lift。
- §1 中列出的 body 内 FQN 引用改为句柄化类型：direct/closure/closure value/gc intrinsic callee 使用 `LirCallableId`，global 使用 `LirGlobalRootKey`，member/dispatch 使用 `LirMemberKey`/`LirDispatchKey`/`LirNominalLayoutKey`，effect op 使用 `ConcreteOpKey`，class ctor 使用 `LirNominalLayoutKey`，runtime nominal metadata 使用 `LirNominalLayoutKey`。
- 新增单测覆盖 direct/closure callable id 与 global/member handle 形状；确认 `crates/scoopc_lir/src/effect_lowered/instruction.rs` 中 `Todo|UnresolvedName` 零命中。
- 验证：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [TODO] T2-07-R：Review T2-07
- 关注点：覆盖 MIR 全部**已实现**的 statement/rvalue/terminator/callkind（无遗漏真实变体）；所有体内引用为句柄/TypeId，无 String FQN；effect 控制（Perform/Handle/Resume/Suspend）保真；**LIR 指令枚举无 `Todo`/`UnresolvedName`/占位**。
- 确认：逐项对照 §1 清单打勾；`grep` LIR 指令定义无占位变体；编译 + clippy。

### [TODO] T2-08：lowering 产出 LIR 指令（state 拥有指令）
- 改 effect-lowering：`LateLoweredState` 拥有 LIR 指令序列，取代 `source_slice: LateLoweredStateSlice`；删 `LateLoweredStateSlice` / `LateLoweredSourceBody`。lowering 从 MIR body 一次性 lift 成 LIR 指令。
- **lift 遇到 MIR `Todo`/`UnresolvedName` 等占位 → 报 unsupported/internal 错误，不产出 LIR 占位。** 这同时验证：完全 lowered 的合法 body 是否还能到达这些占位（理应不能——`UnresolvedName` 在 typecheck 后不应存在，`Todo` 代表未实现特性应在更早报错）。若实测可达，登记为上游（MIR/typecheck）需补的诊断缺口，**不得**在 LIR 层用占位掩盖。
- 验收：`ir.rs` 无 `LateLoweredSourceBody`/`LateLoweredStateSlice`；`LateLoweredProgram` 不再引用 `crate::mir::Body`。

### [TODO] T2-08-R：Review T2-08
- 关注点：lift 忠实（每条 MIR stmt/term 有对应 LIR 指令，语义不变）；state 拥有指令、无回指 MIR；transport metadata 一并 lift。
- 确认：MIR golden / fixture 行为不变；基线绿。

### [TODO] T2-09：codegen 改走 LIR 指令 + 删 `LirArtifact.mir`
- `codegen/mir_body/mod.rs` 的 stmt/rvalue/terminator/callkind walker 改为 walk LIR 指令；删 `LirArtifact.mir: Option<MaterializedMir>`。
- 验收：codegen 无 `crate::mir::Body` 消费；`LirArtifact = { cone, program, base_context→并入 program, object_files }`。

### [TODO] T2-09-R：Review P2c / P2 整体阶段验收
- 关注点（对照 PLAN §3 完成标志）：`LirArtifact` 无 `mir`/`facts`；无 overlay（无 `LateLoweredSourceBody`/`StateSlice`）；无字符串 live key（callable 引用全 `LirCallableId`/`LirCallableHash`）；codegen 单结构 walk。
- 范围纪律：未越界改 P3+；无新增 fallback。
- 确认：`grep -nE "LateLoweredSourceBody|LateLoweredStateSlice|LirArtifact.*mir|LirArtifact.*facts" crates/` 仅余历史/注释；§9 全套基线绿；抽样 diff 可执行行为等价；在 PLAN.md 标记 P2 DONE。

---

## 9. 验证基线（每任务收尾）

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all --all-targets
cargo build -p scoop -p scoopc
python3 tools/dependency_gate.py
python3 tools/spec_fixtures.py check
python3 tools/run_fixtures.py
```

## 10. 风险 / 备注

- **P2c 最大**：指令 lift + 双侧（lowering 产出、codegen 消费）改造，建议在 T2-08/09 间保持小步、每步全 fixture 验。
- `base_context`（类型/布局）并入 `program` 的时机：P2b 处理 layout 时一并规划，避免 P2c 收尾还遗留独立 base_context。
- 跨 cone：`LirCallableHash` 在 `deps` 间解析为本地 `LirCallableId` 的路径在 T2-03 落地，P2c 不应再引入新的跨 cone 字符串匹配。
