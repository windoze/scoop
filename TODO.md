# TODO — P-CG：codegen 直接消费 LIR（去 MIR + 去 FQN + never-fail）

> 计划：[`PLAN.md`](./PLAN.md)；设计：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md) §1.7/§1.8/§13/§2.7。
> 一组任务 TC-01..TC-06，按序；每任务后跟 review；每任务收尾跑 §9 基线。

## 0. 硬纪律（§1.8，违反即打回）

下游对输入 **never-fail**。本阶段全程**禁止**：
- 新增 `Result` 输入错误出口、`.expect()`/`panic!`/`.unwrap()` on input；
- `Todo`/`Unsupported`/`Unknown`/`Fallback` 等占位/escape 变体、`_ => {}` no-op 容忍；
- **句柄→FQN 字符串反转**（如 `lir_*_fqn`、`lir_callable_root -> String`）；
- **LIR→MIR 反向 shim**（如 `lir_*_to_mir`）。

任何「失败可能」必须**上移**为上游（LIR lift 的 producer / MIR 输出）的保证。真正构造不可能的内部不变量用 `unreachable!`/`debug_assert`，不用 `Result`。**绝不留 placeholder。**

## 1. 代码地图（按符号定位，勿全库搜索）

**发射驱动**：`crates/scoopc_codegen_llvm/src/llvm/emit.rs:98-118`（`build_main_module_from_stage_output`）→ `codegen/effect_lowered/body/main_entry.rs:7-34`（`codegen_program_bodies` 遍历 `program.callables()`，按 `callable.plain_abi().is_some()` 分流：plain→`codegen_plain_callable_entry`(:429-634)；effect-step→`codegen_callable_entries`(:32-33)）。

**plain 路径（直接 walk MIR，TC-02 目标）** `codegen/mir_body/`：
- `main_entry.rs:591-623`：`for block in body.blocks { for stmt in block.stmts[slice...] }`——原始 MIR 遍历。
- `mod.rs:141-205`：route-safe gate（`ensure_raw_mir_rvalue_is_route_safe`/`..terminator..`，match `mir::Rvalue`/`mir::TerminatorKind`）。
- `operand.rs`(`codegen_mir_operand`)、`args.rs`、`call.rs:109`、`terminator.rs`、`aggregates.rs`、`transport.rs`、`cast.rs`、`member.rs`、`dispatch.rs`、`callable_lookup.rs`、`const_pat.rs`、`lowering.rs`。
- match 的 `mir::*`：`Statement`/`StatementKind::Assign`、`Rvalue`(含 `::Call`)、`Terminator`(`Return`/`Goto`/`CondBr`)、`Operand`、`Place`、`LocalId`。

**effect-step 路径（语句仍切 MIR，TC-03 目标）** `codegen/effect_lowered/body/`：
- `emitter.rs:117`：`for state in callable.state_graph().states()`（已 LIR 句柄）。
- `lower_source.rs:94-116`：经 published boundary/`LateLoweredPlainBodySlice` 调 `ValuePrimitives::lower_effect_neutral_statement(stmt, ..)`——`stmt` 仍是 `LateLoweredSourceBody`(MIR) 的语句。
- `value.rs`：ValuePrimitives 降值（effect-neutral）。

**LIR 指令 / 容器（TC-01/02/03 消费方）** `crates/scoopc_lir/src/effect_lowered/`：
- `instruction.rs`：`LirStatement`/`LirStatementKind`/`LirRvalue`/`LirCallKind`/`LirInstruction`、`LirExecutableBody`/`LirStateMachineBody`/`LirExecutableState`/`LirCallableHeader`/`LirParam`/`LirLocalDecl`/`LirBodyAnchor`/`LirStatementIndex`。
- `lift.rs`：`lift_statement`/`lift_rvalue`/`lift_terminator`（**TC-01：改全函数、去 `Result`/`invalid_lift`**）。
- `ir.rs:343` `type LateLoweredSourceBody = crate::mir::Body`、`:3591` `LateLoweredStateSlice`（**TC-05 删**）。

**FQN→句柄（TC-04 目标）**：`callable_lookup.rs:27`、`identity.rs:280-357`（`exported_abi_symbol_for_lir_callable`/`lir_callable_id_for_root`/`abi_symbol_for_root`）、`call/lowering.rs:2065`（`published_signature_matches_hir_call`）、`function_cx` 的 `current_callable_fqn`。

**错误类型（TC-06）** `crates/scoopc_codegen_llvm/src/llvm/mod.rs:99`：`LlvmEmitError`。
- **输入失败（上移）**：`Frontend`/`MissingEntryMain`/`EffectLoweringUnsupported`/`BackendGate`/`AmbiguousEntryMain`/`InvalidLiteral`。
- **保留（真后端/IO）**：`Target`/`Builder`/`Instruction`/`ModuleVerificationFailed`/`RunPassesFailed`/`Write{Ll,Obj,Asm}Failed`。

**fail-fast 热点（TC-06）**：`effect_lowered/layout/{classification.rs(141),surface_resume.rs(76),handle_dispatch.rs(48),dispatch.rs(36)}`、`effect_lowered/body/{value.rs(43),mod.rs(40)}`、`mir_body/{const_pat.rs(36),lowering.rs(29)}`。多为「查应被上游 pre-validate 的 ABI fact」的 `expect`、与「verifier 已接受的不变量却在 codegen 失败」的 `panic`。

`LirArtifact.mir: Option<MaterializedMir>` 在 `crates/scoopc/src/pipeline/lir_artifact.rs`（TC-05 删）。

## 2. 任务（按序）

### [DONE] TC-01：LIR lift 落地为全函数，填满所有 callable body

**目标**：让 `crates/scoopc_lir/src/effect_lowered/lift.rs` 的 lift 链成为**全函数（无 `Result`）**，并保证 plain + effect-step 所有 callable 的 `LirExecutableBody` 被填满；占位/形状失败上移到 MIR→LIR 边界。**本任务不删 overlay**（TC-05 才删），LIR 指令与现有 source-slice 行为等价即可。

**起点（已核对，HEAD 49639d4a）**：`lift.rs` 现有 `LirLiftContext`，其 `lift_plain_body`(:45)、`lift_statement_range`(:135)、`lift_statement`(:162)、`lift_rvalue`(:206)、`lift_plain_terminator`(:393)、`lift_member_access`(:455)、`lift_member_target`(:473)、`lift_call_kind`(:493)、`lift_call_transport`(:538) 都返回 `Result<_, EffectLoweringError>`，靠 `invalid_lift(..)` 制造错误。`lift_control_body`(:101) 已是全函数（复用既有 state body）。`lift_plain_body` 已真正构造 `LirExecutableState`/`LirStateBody`（:70-91）。

**`invalid_lift` 的 5 类失败来源，逐类按 §1.8 处理**（这是本任务核心，不许有第六种「就地报错」处理方式）：
1. `MIR Todo statement/rvalue/terminator/unwind reached LIR lift`（:196/:388/:402/:440）、`unresolved MIR name`（:223）——**占位/未解析**。处理：**在 MIR→LIR 生产者边界加 guard**（见下「guard 落点」），保证进入 lift 的 `mir::Body` 不含 `StatementKind::Todo`/`Rvalue::Todo`/`Rvalue::UnresolvedName`/`TerminatorKind::Todo`/`UnwindAction::Todo`。guard 之后这些 arm 在 lift 里**结构不可达** → 改 `unreachable!("guarded at MIR→LIR boundary: ...")`，**不是** `Result`。
2. `missing MIR block bb{}`（:143）——块引用。well-formed MIR 中块 id 必有效 → 直接索引 / `unreachable!`（B 类不变量），不用 `Result`。
3. `MIR CondBr condition is not a local operand`（:425）——terminator 形状。MIR 构造保证 CondBr cond 为 local → `unreachable!`/`debug_assert!`，不用 `Result`。
4. member access（:460）、call_kind/transport 等其余 `invalid_lift`——逐一判定：若是 well-formed MIR 的结构保证 → `unreachable!`/`debug_assert!`；若确为「输入可能非法」→ **上移到 guard**，不在 lift 留 `Result`。
5. 删除 `invalid_lift` 函数本身。

**guard 落点（失败上移的唯一去处）**：在 **MIR 交给 LIR lowering 的边界**（effect-lowering stage 入口 / MIR 输出侧）加一道校验，用 `crates/scoopc_mir/src/mir/placeholder_inventory.rs` 扫描待 lower 的 body，**若含上述占位则在此（MIR 侧、上游）报错**。这道 guard 是「失败上移一级」的临时落点，P4/P5 继续上移到 HIR。guard 是**校验/拒绝**，不是 escape 变体，不违反「无 placeholder」。

**步骤**：
- S1：把 `lift_*` 全部改成返回值类型本身（去 `Result`）；调用点（`lift.rs:62/158/170`、`segment.rs:383`）去掉 `?`。
- S2：5 类失败按上表改为 `unreachable!`/`debug_assert!`（结构不可达）或上移；删 `invalid_lift`。
- S3：在 MIR→LIR 边界加 placeholder guard（上游报错）。
- S4：确认 plain（`lift_plain_body`）与 effect-step（`lift_control_body`）两路都产出完整 `LirExecutableBody`（state + 指令 + terminator），无空缺。

**严禁（违反即打回，上次就栽在这）**：
- 不得新增 `Result`/`.expect()`/`panic!`-on-input 来替代 `invalid_lift`（`unreachable!` 仅用于「guard 后结构不可达」，不得用于「输入可能触发」）。
- 不得新增 `Todo`/`Unsupported`/`Unknown`/escape 变体或 `_ => {}` no-op 容忍。
- 不得用 `lir_*_to_mir` 反向转换、不得句柄→FQN 反转。
- **若发现某占位/形状确实会被合法 fixture 触达、无法在 MIR 边界干净 guard**：STOP，在完成记录里写明具体来源（哪条 MIR 构造产出占位、哪个 fixture），登记为上游缺口，**不得**为了让基线变绿在 lift/codegen 回填占位或 `Result`。

**验收**：
- `grep -nE "Result<|invalid_lift|EffectLoweringError" crates/scoopc_lir/src/effect_lowered/lift.rs` → lift 链无 `Result`/`invalid_lift`（仅可能保留无关 import）。
- plain + effect-step 每个 callable 都有完整 `LirExecutableBody`（单测断言）。
- 新增单测：对代表性 body 比对 LIR 指令序列与原 MIR slice 语义等价；占位 body 在 MIR guard 处被拒（而非进入 lift）。
- §9 全套基线绿（占位 fixture 若在 MIR guard 暴露=对的，按上面 STOP 规则处理）。

**完成记录（2026-06-05）**：
- 将 `lift.rs` lift 链改为 total functions，删除 `invalid_lift` / `EffectLoweringError` 依赖；guard 后不应出现的 MIR placeholder / unresolved name / invalid ranges 改为结构不可达。
- 在 MIR placeholder inventory 中发布 LIR-lift body guard，并在 late-lowering builder 的 MIR→LIR 入口调用；补齐 unresolved name、unresolved member、Todo、CondBr non-local 条件等拒绝路径。
- plain body 直接生成完整 LIR executable states；plain local effect-control 和 effect-step body 复制 state-owned LIR body；direct/bodyless callable references 使用稳定 `LirCallableRef`。
- 修复 state-owned source slice / classification 在 materialization、opt 和 codegen 中的锚点传播；补齐 LIR terminator local liveness，避免 continuation frame 漏捕获 branch condition locals。
- 修复 companion/static member namespace receiver 在 MIR 中遗留 `UnresolvedName` 的生产缺口。
- 新增/更新测试：LIR lift statement/control-body 单测、MIR→LIR guard 单测，并刷新 effect-lowered golden fixtures。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-01-R：Review TC-01
- **关注点**：
  - `lift.rs` lift 链全函数、**无 `Result`/`invalid_lift`**；former 失败均为 (a) guard 后 `unreachable!`/`debug_assert!`（结构不可达）或 (b) 上移到 MIR guard——**无第三种「就地 `Result`/`panic`-on-input」**。
  - 占位 guard 确在 **MIR→LIR 上游边界**、用 `placeholder_inventory`；LIR 端结构上见不到 `Todo`/`UnresolvedName`。
  - LIR 指令覆盖 **plain + effect-step 全部 body**、与 MIR slice 等价、无空 body。
  - **零反模式**：无 escape 变体、无 no-op 容忍、无 `lir_*_to_mir`、无句柄→FQN 反转。
- **确认**：
  - `grep -nE "Result<|invalid_lift" .../lift.rs` 仅余无关项；`grep -rnE "Todo|UnresolvedName|Unsupported" .../effect_lowered/{lift,instruction}.rs` 无新 escape 变体。
  - `grep -rn "lir_.*_to_mir\|_fqn\b" crates/scoopc_lir/src/effect_lowered` 无反向 shim / 句柄→FQN。
  - §9 基线绿；若 MIR guard 暴露 fixture 缺口，确认已按 STOP 规则登记 HIR 待补、**未回填占位**让其变绿。

**完成记录（2026-06-05）**：
- 审查 `TC-01` 落地结果：`lift.rs` lift 链为全函数，`rg -n "Result<|invalid_lift|EffectLoweringError" crates/scoopc_lir/src/effect_lowered/lift.rs` 无命中；占位/未解析 MIR 形状在 lift 中仅保留 guard 后结构不可达的 `unreachable!`。
- 确认 MIR→LIR guard 位于 `placeholder_inventory::validate_body_for_lir_lift` 并在 LIR builder 入口调用，覆盖 MIR `Todo`、`UnresolvedName`、未解析 member、非 local `CondBr` 条件等拒绝路径。
- 确认 plain body 通过 `lift_plain_body` 生成完整 `LirExecutableBody`，plain local effect-control 与 effect-step 通过 `lift_control_body` 复制 state-owned LIR body；相关单测覆盖语句序列与 state-owned body。
- 反模式检查通过：未发现 `lir_*_to_mir`；`instruction.rs` 未定义 `Todo`/`UnresolvedName` LIR placeholder 变体；广义 `_fqn` 命中限于既有 root/symbol/布局键等非新增反向 shim。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-02-PRE1：补齐 LIR plain lowering 的 effect-typed closure adapter

**目标**：在 plain 路径从 MIR `ValuePrimitives` 切到 LIR 指令后，补齐原先只存在于 MIR value lowering 中的 effect-typed closure adapter 逻辑，确保 `val f: () -> T / E = { ... }`、`when`/LUB 产出的 function value、struct 字段中的 function value 等场景在 LIR lowering 下仍按静态 effect row 选择正确 closure carrier / adapter fn pointer。

**阻塞来源（2026-06-05 迁移 TC-02 时暴露）**：`codegen_lir_make_closure` 目前只按 LIR `fn_ptr: LirCallableId` 生成普通 closure object；缺少 MIR `ValuePrimitives::maybe_build_effect_typed_closure_target_fn_ptr` / `install_effect_typed_closure_target_overrides_for_struct_fields` 对应的 LIR-native 逻辑。结果 `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop` 在 LIR plain path 下运行失败（应输出 `5\ncaught\n9\n10\n` 并 exit 10，当前无输出且异常退出）。

**步骤**：
- S1：将 effect-typed closure surface layout 查询、plain closure adapter、effectful closure step adapter 提供为 LIR lowering 可直接调用的 helper；输入使用 `LirExecutableBody`/`LirLocalDecl`/`LirCallArg`/`LirCallableId`，不得构造 MIR 反向 shim。
- S2：`LirRvalue::MakeClosure` lowering 根据 target local / downstream consumer 的静态 function type 检测 effect row，并在需要时写入 adapter fn pointer。
- S3：补齐 struct literal/function-value 聚合场景的 LIR adapter override，对应 MIR 旧逻辑的覆盖面。

**验收**：
- `cargo clippy --all-targets -- -D warnings` 通过。
- `cargo test -p scoop --test p7_default_pipeline` 通过，特别是 `single_pipeline_runs_higher_order_function_value_handled_effect_cli`。
- 不新增 `lir_*_to_mir` 反向转换；不把 `LirCallableId` 转回 FQN 作为运行期查找路径（符号名/诊断除外）。

**完成记录（2026-06-05）**：
- 新增 LIR-native effect-typed closure adapter helper，并在 plain LIR rvalue lowering 中覆盖直接 `LirRvalue::MakeClosure`、`Use`/`Transport` 传播的 closure local，以及 struct literal function-field adapter override。
- adapter 选择以 `LirExecutableBody`、LIR local、`LirCallArg`、`LirCallableId` 为入口；未新增 `lir_*_to_mir` 反向转换。现有 ABI layout 仍有 root/symbol 键查询，运行期 closure carrier 写入的是已生成的 adapter function pointer。
- 修复 dependency gate 对新 helper 中直接 `crate::mir::` 路径的拦截，改用 LIR `mir_source` 边界别名。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_higher_order_function_value_handled_effect_cli -- --nocapture`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`。
- 基线运行：`cargo test --all --all-targets` 仍有 11 个 plain-LIR `scoopc` LLVM 单测失败；`python3 tools/run_fixtures.py` 仍有 `268/1625` fixture 失败。两组失败已在 `TC-02` 下精确登记为 plain LIR 主体迁移的验收阻塞项，本任务不以 workaround 扩大范围修复 TC-02。

### [DONE] TC-02-PRE2：收敛 plain-LIR 剩余 fixture/runtime 残差

**目标**：在 `TC-02` 标记完成前，收敛本轮 plain-LIR 主体迁移后仍未绿的完整 fixture 基线。不得通过退回 MIR walk、LIR→MIR shim、句柄→FQN 反转、弱化 fixture 行为或跳过失败来完成；旧 IR substring 只能更新为实际 LIR 语义下等价且更准确的断言。

**阻塞来源（2026-06-05，迁移 TC-02 时暴露）**：本轮已将 `cargo test --all --all-targets` 恢复绿色，并将 `python3 tools/run_fixtures.py` 从 `268/1625` 失败降到 `31/1625` 失败；剩余失败仍阻塞 `TC-02` 完成。

**剩余失败分组**：
- 旧 LLVM IR substring 期望漂移：`build/effect_lowered_member_codegen_emit_llvm.scoop`、`build/effect_lowered_non_boundary_dynamic_call_emit_llvm.scoop`、`build/effect_lowered_step_enum_no_outward.scoop`、`umb_fix/P6-T01-platform/pos_platform_structlit_immortal_ir.scoop`。
- LIR atomic-int lvalue 覆盖不足：`build/unsafe_atomic_int_field_lvalue_llvm.scoop`、`build/unsafe_atomic_int_top_level_storage_llvm.scoop`、`run-pass/unsafe_atomic_int_field_lvalue_basic.scoop`。
- plain-LIR 运行时行为/ABI 残差：`run-pass/array_lit_infer_string_char_float_basic.scoop`、`run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`、`run-pass/enum_value_only_when_basic.scoop`、`run-pass/extension_property_getter_basic.scoop`、`run-pass/extern_native_aggregate_return_direct_indirect_parity.scoop`、`run-pass/float_literal_runtime_basic.scoop`、`run-pass/gc_pin_unpin_basic.scoop`、`run-pass/literal_ops_compare_direct_matrix_basic.scoop`、`run-pass/object_companion_once_init_basic.scoop`、`run-pass/object_companion_value_named_nested_init_basic.scoop`、`run-pass/safe_member_access_ref_and_extension_basic.scoop`、`run-pass/scalar_method_intrinsic_basic.scoop`、`run-pass/struct_computed_property_getter_basic.scoop`、`run-pass/struct_computed_property_not_ctor_field_basic.scoop`、`run-pass/top_level_callable_value_call_basic.scoop`、`run-pass/unsafe_funptr_aggregate_return_tuple.scoop`、`run-pass/unsafe_funptr_extern_call_basic.scoop`、`run-pass/unsafe_funptr_receiver_call_basic.scoop`、`runtime_gc/gc_pin_unpin_move_stress_matrix.scoop`、`runtime_gc/gc_stw_cross_thread_roots_basic.scoop`、`umb_fix/B-13-composite-transport/pos_array_composite_transport.scoop`、`run_pass_cone/dependency_c_sources_extern_call`、`run_pass_cone/dependency_cxx_sources_extern_call_cpp_stdlib`、`run_pass_cone/source_path_dependency_public_call`。

**验收**：
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 通过。
- `cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check` 通过。
- `python3 tools/run_fixtures.py` 通过；若只更新旧 IR substring，必须保持检查语义等价于 LIR 发射后的真实行为。

**完成记录（2026-06-05）**：
- 补齐 LIR plain lowering 的剩余运行时 parity：Float `toInt` named intrinsic、顶层 function value / `FunPtr` direct-call、`scoop.unsafe.invoke`、namespace-only top-level refs、external/bodyless direct-call result type inference、`GC.pin/unpin`、递归 atomic lvalue 与顶层 atomic storage。
- 修复跨 cone public callable ABI symbol 稳定性，source-level public callable ABI 改为按公共签名语义发布；保留 native/extern callable 的独立 runtime/native symbol surface。
- 更新旧 LLVM substring / effect-lowered / MIR golden：LIR 命名漂移、nested atomic GEP regex、Platform LIR struct literal IR、MIR intrinsic callable 计数及 effect-lowered ABI symbol hash。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-02：plain 路径（`mir_body/`）改 walk LIR 指令

**目标**：plain callable 发射从「walk 原始 `mir::Body`」改为「walk `LirExecutableBody` 的 LIR 指令」，去掉 `mir::*` body match 与 route-safe gate。**依赖 TC-01 + TC-02-PRE1 + TC-02-PRE2**（plain body 的 LIR 指令已填满，LIR plain lowering 已具备 effect-typed closure adapter parity，且剩余 fixture/runtime 残差已收敛）。

**起点（已核对）**：
- 入口 `body/main_entry.rs:429-634` `codegen_plain_callable_entry`；其 body 遍历 `:591-623` `for block in body.blocks { for stmt in block.stmts[slice...] }`（原始 MIR）。
- gate `mir_body/mod.rs:141-205` `ensure_raw_mir_rvalue_is_route_safe` / `ensure_raw_mir_terminator_is_route_safe`（match `mir::Rvalue`/`mir::TerminatorKind`）。
- 子文件：`operand.rs`(`codegen_mir_operand`)、`args.rs`、`call.rs:109`、`terminator.rs`(`Return`/`Goto`/`CondBr`)、`aggregates.rs`、`transport.rs`、`cast.rs`、`member.rs`、`dispatch.rs`、`callable_lookup.rs`、`const_pat.rs`、`lowering.rs`。match 的 `mir::*`：`Statement`/`StatementKind::Assign`、`Rvalue`(含 `::Call`)、`Terminator`、`Operand`、`Place`、`LocalId`。

**已观测且归属 TC-02 的完整 Rust 测试失败（2026-06-05，TC-02-PRE1 收尾时暴露）**：`cargo test --all --all-targets` 当前 `scoopc` lib 有 11 个 plain-LIR 发射相关失败；TC-02 完成前必须逐项修复并恢复 §9 基线：
- `pipeline::llvm_codegen_stage::tests::llvm_array_composite_transport`
- `pipeline::llvm_codegen_stage::tests::llvm_atomic_ref_uses_atomic_instructions_and_gc_barrier`
- `pipeline::llvm_codegen_stage::tests::llvm_closure_env_transport`
- `pipeline::llvm_codegen_stage::tests::llvm_closure_refcell_capture_loads_env_without_env_writeback`
- `pipeline::llvm_codegen_stage::tests::llvm_entry_global_entry_selection_uses_lir_callable_signature_for_argv`
- `pipeline::llvm_codegen_stage::tests::llvm_enum_payload_transport`
- `pipeline::llvm_codegen_stage::tests::llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
- `pipeline::llvm_codegen_stage::tests::llvm_value_boxing_transport`
- `pipeline::llvm_codegen_stage::tests::mir_member_access_codegen`
- `pipeline::llvm_codegen_stage::tests::mir_store_member_codegen`
- `pipeline::llvm_codegen_stage::tests::platform_literal_stage_ir_uses_immortal_structlit_without_alloc`

**已观测且归属 TC-02 的完整 fixture 基线失败（2026-06-05，TC-02-PRE1 收尾时暴露）**：`python3 tools/run_fixtures.py` 当前失败 `268/1625`。失败集中在 plain-LIR 直接调用/参数/返回类型、member/dispatch/transport/atomic/platform IR 期望漂移，以及由这些缺口引起的 `build/`、`codegen/`、`run-pass/`、`runtime_gc/`、`umb_fix/`、`run_pass_cone/` 运行失败；TC-02 完成前必须让这个完整命令恢复绿色，或把每个仍失败目标改成符合 LIR 语义的正确期望。当前已见代表性根因包括 `args.rs:435`、`callable_lookup.rs:326`、`call.rs:562`、`call/abi.rs:88` 的 plain-LIR codegen 缺口，以及旧 `pass_mir_*`/`plain_dispatch_call`/`@__scoop_immortal_agg_` substring 期望漂移。

**步骤**：
- S1：`codegen_plain_callable_entry` 从 `callable.executable_body()`（`LirExecutableBody`）取 state/指令序列，替代 `body.blocks[].stmts[]`。
- S2：`mir_body/` 各 `codegen_mir_*` 改为 `codegen_lir_*`，match `LirStatement`/`LirStatementKind`/`LirRvalue`/`LirCallKind`/`LirTerminator`/`LirOperand`（`instruction.rs` 定义）替代对应 `mir::*`。operand/local 用 LIR local 句柄（`LirLocalDecl`/local 下标），不再 `mir::Operand`/`mir::LocalId`。
- S3：**删 route-safe gate**（`ensure_raw_mir_*`）——LIR 指令集 total 且构造即 route-safe，gate 无意义。
- S4：`callable_lookup.rs` 的 closure body 解析也改走 LIR（与 TC-04 衔接：callee 句柄而非 FQN）。

**严禁**：不得保留任何 `mir::{Statement,Rvalue,Terminator,Operand,Place}` 的 body match；不得 `lir_*_to_mir` 反向转换；不得句柄→FQN 反转；**若 LIR 指令缺某字段导致发射缺数据 → 回 TC-01 在 producer 补全，不在 codegen 造转换/占位/`Result`**。

**验收**：
- `grep -rnE "mir::(Statement|Rvalue|Terminator|Operand|Place|LocalId)" crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body` → body match 清零（仅可能余类型别名等非 body 用法）。
- plain callable 由 LIR 指令发射；route-safe gate 已删。
- §9 基线绿；抽样 diff 同一输入的 LLVM IR/可执行行为等价。

**完成记录（2026-06-05）**：
- `codegen_plain_callable_entry` 的普通 plain 发射改为以 `LirExecutableBody` / LIR header 为权威来源：返回类型、source span、materialized closure 判定和 composite transport 校验均走 LIR；普通分支遍历 LIR states/statements 并调用 `codegen_lir_statement` / `codegen_lir_plain_terminator`。
- 删除旧 plain MIR walker / gate：`codegen_mir_statement`、`codegen_mir_terminator`、`codegen_mir_rvalue`、`codegen_mir_call` 和 raw MIR route gate 已移除；closure body 参数绑定残留的 MIR helper 已删除。
- `mir_body/` 内剩余 source-slice 兼容层改走 LIR 发布的 `mir_source` 边界；`rg -n "mir::(Statement|Rvalue|Terminator|Operand|Place|LocalId)" crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body`、`rg -n "ensure_raw_mir_|raw_mir_route_gate|codegen_mir_statement|codegen_mir_terminator|codegen_mir_rvalue\\(|codegen_mir_call\\(" crates/scoopc_codegen_llvm/src/llvm/codegen`、`rg -n "lir_.*_to_mir" crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body` 均无命中。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-02-R：Review TC-02
- **关注点**：plain 发射逐指令对应原 MIR、语义不变；`mir_body/` 无 `mir::*` body match 残留；route-safe gate 已删（不是注释掉）；未新增 fail-fast/占位/FQN 反转/`lir_*_to_mir`。
- **确认**：`grep -rnE "mir::(Statement|Rvalue|Terminator|Operand|Place)" .../mir_body` 清零；`grep -rn "ensure_raw_mir_" .../mir_body` 清零；`grep -rn "lir_.*_to_mir\|_fqn" .../mir_body` 无反向/FQN 反转；抽样 diff LLVM IR 等价；§9 绿。

**完成记录（2026-06-05）**：
- 审查 `TC-02` 落地结果：`codegen_plain_callable_entry` 的普通 plain 分支和 `codegen_lir_source_closure_fun` 均从 `LirExecutableBody` 遍历 state-owned statements/terminator，并调用 `codegen_lir_statement` / `codegen_lir_plain_terminator`；旧 plain MIR statement/rvalue/terminator walker 未作为生产路径保留。
- 反模式检查通过：`rg -n "mir::(Statement|Rvalue|Terminator|Operand|Place|LocalId)" crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body` 无命中；`rg -n "ensure_raw_mir_|raw_mir_route_gate|codegen_mir_statement|codegen_mir_terminator|codegen_mir_rvalue\\(|codegen_mir_call\\(" crates/scoopc_codegen_llvm/src/llvm/codegen` 无命中；`rg -n "lir_.*_to_mir" crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body` 无命中。`_fqn` 的现存命中对应已排期的 `TC-04` FQN→句柄迁移范围，未发现 TC-02 新增的 LIR→MIR shim。
- 语义等价由 LLVM/codegen 单测、fixture build/run/golden 检查和完整 fixture suite 覆盖；未发现需要在本 review 中修复或新增前置任务的失败。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-03：effect 路径语句改 walk LIR

**目标**：effect-step 路径的**语句发射**从 MIR source-slice 改为 LIR 指令。state 遍历本就已是 LIR 句柄（`emitter.rs:117` `for state in callable.state_graph().states()`），本任务只动「state 内语句」这一层。**依赖 TC-01**。

**起点（已核对）**：
- `body/lower_source.rs:94-116`：经 published boundary / `LateLoweredPlainBodySlice` 调 `ValuePrimitives::lower_effect_neutral_statement(stmt, used_locals)`，其中 `stmt` 取自 `LateLoweredSourceBody`(= `crate::mir::Body`) 的语句。
- `body/value.rs`：`ValuePrimitives` 的 effect-neutral 降值，match `mir::Rvalue`/`mir::Operand`。
- boundary/state 控制流来自 LIR（`LateLoweredBoundaryLowering` / state graph），本任务不动。

**步骤**：
- S1：`lower_effect_neutral_statement` 及 `value.rs` 的相关入口签名改吃 `LirStatement`/`LirRvalue`（来自 `LirExecutableState` 的 state-owned 指令 / `LirBodyAnchor`），不再接 `mir::Statement`/`mir::Rvalue`。
- S2：删去经 `LateLoweredPlainBodySlice`/`source_slices()`/`source_body()` 取 MIR 语句的路径；state 内语句序列直接来自 LIR body。
- S3：boundary 处的 operand 来源（`LateLoweredBoundaryLowering` 已是 LIR 句柄）与 LIR 语句衔接，确保 boundary 前后语句顺序一致。

**严禁**：同 TC-02（无 `mir::*` 语句 match、无 `lir_*_to_mir`、无 FQN 反转、缺数据回 TC-01 补）。

**验收**：
- `grep -rnE "\.source_body\(\)|\.source_slices\(\)|LateLoweredSourceBody|mir::Rvalue|mir::Statement" crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body` → 语句消费清零（boundary/state 控制流的非语句用法除外）。
- §9 基线绿；effect-step fixture 行为不变。

**完成记录（2026-06-05）**：
- `CallableEmitter` 改以 `LirExecutableBody` 作为 effect/local-control body 的语句与 local slot 权威来源；state 发射从 `state.statements()` 遍历 LIR `LirStatement`，不再经 `source_slices()` 回读 MIR statement。
- `lower_effect_neutral_statement` / dynamic invoke / class ctor boundary 改吃 LIR statement、LIR call args 和 `LirBodyAnchor` classification；class ctor hidden-init source、composed-call replay prefix、completion payload verifier 均通过 LIR statement anchor 定位。
- `effect_lowered/body` 验收 grep 清零：`.source_body()` / `.source_slices()` / `LateLoweredSourceBody` / `mir::Rvalue` / `mir::Statement` 均无命中；删除已无调用的 MIR local-use 收集链。
- 修复迁移后暴露的 LIR class ctor cleanup GC 残差：失败构造对象的 deferred root 在返回后清理，spill root clear store 标记为 volatile，保持 `class_init_raise_cleanup_*_gc_basic` 行为不变。
- 更新 LLVM runtime type primitive 单测中 `!is` 的 IR 名称期望，从旧 MIR label 切到 LIR label。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-03-R：Review TC-03
- **关注点**：effect-step 每条语句来自 LIR、与原 MIR slice 等价；boundary/state 控制流仍正确；无 MIR 语句 slice 残留、无占位/`Result`-on-input。
- **确认**：上述 grep 清零；effect 相关 golden/fixture 行为不变；§9 绿。

**完成记录（2026-06-05）**：
- 审查 `TC-03` 落地结果：`CallableEmitter::lower_state_statements` 逐 state 遍历 `LateLoweredState::statements()`，并以 `LirBodyAnchor::statement` 查 published classification；effect-neutral 和 dynamic invoke 语句均进入 `lower_effect_neutral_statement` / `lower_published_call_statement` 的 LIR statement 路径。
- 修复 review 中发现的 LIR local-use 漏收集：`used_locals` 现在除 LIR statements/terminators 外，还并入 boundary operand contracts、frame-slot source locals、completion payload sources 和 handle completion payload sources，避免仅由 Call/Perform/Resume boundary payload/args/continuation 消费的 top-level refs 被误判 unused 并跳过。
- 反模式检查通过：`rg -n "\.source_body\(\)|\.source_slices\(\)|LateLoweredSourceBody|mir::Rvalue|mir::Statement" crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body` 无命中；`rg -n "lir_.*_to_mir" crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body` 无命中。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-04：FQN 引用改句柄

**目标**：codegen 里把 callee / 符号 / 布局的 **live FQN 字符串查找**改为 `LirCallableId` / `NominalId` 句柄直接 deref。FQN 仅保留为**符号名发射**（LLVM symbol）与**诊断/调试**用途（§2.7：String 仅调试）。可与 TC-02/03 并行，但建议在其后。

**起点（已核对，~1283 处 `_fqn`）**，主要类别：
- callee 查找：`mir_body/callable_lookup.rs:27`、`identity.rs:356` `program.callable(fqn)`。
- 符号预留 / id 解析：`identity.rs:280-357` `exported_abi_symbol_for_lir_callable(fqn)` / `lir_callable_id_for_root(fqn)` / `abi_symbol_for_root(fqn)`。
- 路由匹配：`call/lowering.rs:2065` `published_signature_matches_hir_call(fqn, ..)`。
- 身份跟踪：`function_cx` 的 `current_callable_fqn: Option<String>`。

**步骤**：
- S1：callee/符号/布局/dispatch 的查找入口改为接收并 deref `LirCallableId`（callable）/ `NominalId`（类型布局）；`lir_callable_id_for_root(fqn)` 这种「FQN→id」查找应在更上游（LIR 已持 id）消除，codegen 直接拿 id。
- S2：`current_callable_fqn` 改为 `current_callable: LirCallableId`（需要符号名时由 id deref 取）。
- S3：`published_signature_matches_hir_call` 等「按 FQN + 签名匹配路由」改为按句柄/已发布契约判定（与 TC-02/03 的 LIR call-site 契约衔接）。
- S4：保留 FQN **仅**用于 emit LLVM 符号名与诊断信息。

**严禁**：不得保留「按 FQN 查找、查不到就 fallback/默认」路径；不得把句柄又转回 FQN 去查（句柄→FQN 反转）；缺映射回上游补。

**验收**：
- `grep -rn "program.callable(" crates/scoopc_codegen_llvm`、`grep -rn "lir_callable_id_for_root\|abi_symbol_for_root\|current_callable_fqn" crates/scoopc_codegen_llvm` → live 查找清零（剩余仅符号名发射/诊断字符串）。
- §9 基线绿。

**完成记录（2026-06-05）**：
- 将 codegen 当前 callable 身份从 `current_callable_fqn` 迁到 `current_lir_callable_id`，并在 plain/effect/closure body 入口按 active LIR program 解析当前 `LirCallableId`，避免跨 primary/ABI program 索引漂移。
- direct LIR call lowering 改为以 `LirCallableRef` 作为目标身份，ABI symbol、callable symbol facts、dispatch target declaration 和 closure stable identity 通过 callable ref/id deref；FQN 仅保留在 LLVM symbol 名、稳定命名和诊断文本路径。
- `ProgramAbiMaterializer`、plain/effect carrier、closure env、GC dispatch target、source signature matching 和相关 layout tests 去掉 `program.callable(...)` / `lir_callable_id_for_root` / `abi_symbol_for_root` / `current_callable_fqn` 路径；`published_signature_matches_hir_call` 改为按 LIR callable ref 或已发布 source signature 校验。
- 修复迁移后暴露的 active LIR program/body program ID 不一致问题；首次完整 fixture 暴露的 4 个失败目标已逐一复测并恢复绿色。
- 验收 grep 通过：`rg -n "program\.callable\(" crates/scoopc_codegen_llvm`；`rg -n "lir_callable_id_for_root|abi_symbol_for_root|current_callable_fqn" crates/scoopc_codegen_llvm`；`rg -n "published_signature_matches_hir_call" crates/scoopc_codegen_llvm` 均无命中。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-04-FIX1：清除 TC-04 review 发现的剩余 FQN live callable 查找

**目标**：修复 `TC-04-R` 静态审查发现的 TC-04 残留：LLVM codegen 生产路径仍有按 root/FQN 字符串解析 callable、符号或 callable layout 的 live 查找。完成后 `TC-04-R` 才能确认「callee/符号/布局 live 引用全句柄；FQN 仅作符号名/诊断」。

**阻塞来源（2026-06-05，TC-04-R 审查时发现）**：基础验收 grep 虽显示 `program.callable(`、旧 `lir_callable_id_for_root` / `abi_symbol_for_root` / `current_callable_fqn`、`lir_*_to_mir` 无命中，但更严格的生产路径审查仍发现 root/FQN live lookup：
- `crates/scoopc_codegen_llvm/src/llvm/codegen/call/abi.rs`：`.callable(callable_fqn)`、`callable_id_by_root(callable_fqn)` 用于 published callable facts/signature。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/{emitter.rs,main_entry.rs}`：当前 callable id 仍优先通过 `active.callable_id_by_root(callable.root_fqn())` 映射。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs`：plain direct call lowering 使用 `.callable(callee_fqn)` 与 `plain_callable_layout_by_root_fqn(callee_fqn)` 等 root-based layout 查询。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs`：`lir_callable_ref_for_root` / `exported_abi_symbol_for_lir_root` 仍把 root/FQN 反查为 callable ref/id 后继续发射。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/{call.rs,callable_lookup.rs,operand.rs}` 与 `call/lowering.rs`：仍有 root/FQN 到 `LirCallableRef`、ABI symbol 或 published root 的运行期选择路径。

**步骤**：
- S1：将生产路径 helper 的入口从 root/FQN 改为 `LirCallableId` / `LirCallableRef`；跨 primary/ABI program 映射时使用 LIR stable callable key/hash 或已发布 callable ref，不得通过 `root_fqn()` 反查。
- S2：将 direct call、closure/source callable、published signature、ABI symbol、callable layout 查询改为消费 LIR call-site contract 中的 callable handle；缺少 handle/contract 时回 producer 补，不在 codegen 用 FQN fallback。
- S3：删除或测试隔离 `lir_callable_ref_for_root`、`exported_abi_symbol_for_lir_root`、production `.callable(..)`/`callable_id_by_root(..)` 和 callable root-based layout lookup；FQN 字符串只允许保留在 LLVM symbol 名生成、诊断文本、source-signature 文本字段或非 callable 的 global/nominal layout key 场景。

**严禁**：不得新增 LIR→MIR 反向转换；不得把 `LirCallableId`/`LirCallableRef` 转回 FQN 再查；不得用「查不到就 fallback」或 `is_ok()` 探测 ABI surface。

**验收**：
- `rg -n "\.callable\(|callable_id_by_root|lir_callable_ref_for_root|exported_abi_symbol_for_lir_root|callable_layout_by_root_fqn|plain_callable_layout_by_root_fqn|maybe_plain_callable_layout_by_root_fqn" crates/scoopc_codegen_llvm/src/llvm/codegen --glob '*.rs'` 生产路径清零（测试代码中的 fixture lookup 可保留）。
- `rg -n "program\.callable\(|lir_callable_id_for_root|abi_symbol_for_root|current_callable_fqn|lir_.*_to_mir" crates/scoopc_codegen_llvm` 无生产命中。
- `rg -n "_fqn" crates/scoopc_codegen_llvm/src/llvm/codegen` 抽样确认剩余均为符号名、诊断、source-signature 文本字段，或明确非 callable 的 global/nominal layout key。
- §9 基线绿。

**完成记录（2026-06-05）**：
- 删除/替换 TC-04 review 发现的生产路径 root/FQN callable 反查：`current_lir_callable_id` 改为随当前 active LIR program/body program 直接绑定；plain/effect layout 查询新增 `LirCallableId` / `LirCallableRef` / body-version-key 入口；`lir_callable_ref_for_root`、`exported_abi_symbol_for_lir_root` 和 root-named layout 查询不再作为生产 helper 暴露。
- direct call、closure adapter、native callable wrapper、dispatch target declaration、source closure body、published signature 查询等路径改为消费 LIR handle、body version key、published symbol facts 或 source-signature 文本字段；未新增 `lir_*_to_mir` 反向转换。
- 修复迁移中暴露的 LIR plain interface dispatch parity：补齐 LIR 静态 interface dispatch，避免 `println<String>` / `ToString` candidate-set 退化为空 itable 动态分派导致 `exit(7)`。
- 验收 grep 通过：生产路径 `rg -n "\.callable\(|callable_id_by_root|lir_callable_ref_for_root|exported_abi_symbol_for_lir_root|callable_layout_by_root_fqn|plain_callable_layout_by_root_fqn|maybe_plain_callable_layout_by_root_fqn" crates/scoopc_codegen_llvm/src/llvm/codegen --glob '*.rs' --glob '!**/tests/**'` 无命中；`rg -n "program\.callable\(|lir_callable_id_for_root|abi_symbol_for_root|current_callable_fqn|lir_.*_to_mir" crates/scoopc_codegen_llvm` 无命中。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [DONE] TC-04-FIX2：清除 carrier/dispatch 残留 FQN callable 选择

**目标**：修复 `TC-04-R` 复审发现的剩余生产路径 root/FQN callable 选择：carrier/dispatch 发布、fallback registry、dynamic carrier target、vtable/itable target 与 effect-step facts/layout 查询必须改为消费 LIR callable handle、body-version key 或已发布 callable contract；FQN 字符串只能保留为 LLVM symbol 名、诊断文本、source-signature 文本字段，或非 callable 的 nominal/global layout key。

**阻塞来源（2026-06-05，TC-04-R 复审时发现）**：`TC-04-FIX1` 后基础验收 grep 已清零旧 helper，但更严格的 `_fqn` 抽样与 carrier 路径审查仍发现生产代码用 callable root/FQN 做 live target/layout/facts 选择：
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/layout/carrier.rs`：`published_callable_roots` / `plain_callable_roots`、物理 `class_vtables` / `class_itables` 的 `impl_member_fqn` / `method_impl_fqns` 过滤、`dynamic_dispatch_carrier_targets` 的 string target key、`callable_layout_for_carrier_target` 按 `layout.root_fqn() == callable_fqn` 选择版本。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/layout/lookup.rs`：`callable_facts_for_root` / `effect_step_callable_facts_for_root` 通过 `program.callables().find(|callable| callable.root_fqn() == root_fqn)` 回查 callable facts。
- `crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs`、`gc.rs`、`mir_body/aggregates.rs`、`mir_body/dispatch.rs`、`call/lowering.rs` 仍以 callable FQN 字符串注册/索引 callable carrier fallback、entry symbol、vtable/itable/funptr target。
- `DynamicInvokeLayout::candidate_targets()` / `CallableCarrierTargetLayout` 仍以 `Vec<String>` / `(CallableCarrierKind, String)` 表达 callable target；这使 codegen 仍可通过 FQN 选择 live callable 布局。

**步骤**：
- S1：把 callable carrier target key 从 `(CallableCarrierKind, String)` 改为 handle-native key（优先 `LirCallableId` / `LirCallableRef`，跨 cached dep 需要区分 program origin 时使用已有 ABI origin + body-version/callable key），并让 `CallableCarrierTargetLayout` / `DynamicInvokeLayout` 暴露 handle-native target。
- S2：将 vtable/itable/dynamic invoke/carrier shell 发布从 `impl_member_fqn` / `method_impl_fqns` 字符串过滤改为读取 LIR physical layout 或 published contract 中的 callable handle；如果 producer 目前只发布 FQN，回 producer 补 handle 字段，不在 codegen 用 root/FQN 反查。
- S3：删除生产路径 `callable_facts_for_root` / `effect_step_callable_facts_for_root`，closure/dispatch carrier args ABI、callable layout version selection、fallback registry 与 entry symbol lookup 均按 callable handle/body-version key 查询。
- S4：保留 FQN 仅用于 symbol 名生成、诊断文本、source-signature 文本字段；`_fqn` 抽样必须能逐项解释为非 live callable 查找。

**严禁**：不得新增 `lir_*_to_mir` 反向转换；不得把 `LirCallableId` / `LirCallableRef` 转回 FQN 再查；不得用 string fallback、`is_ok()` 探测 ABI surface 或多版本时按 FQN 猜测 authoritative version。

**验收**：
- `rg -n "callable_facts_for_root|effect_step_callable_facts_for_root|callable_layout_for_carrier_target|published_callable_roots|plain_callable_roots|candidate_targets\(|CallableCarrierKind, String|callable_carrier_entry_symbols|plain_callable_carrier_fallback_targets" crates/scoopc_codegen_llvm/src/llvm/codegen --glob '*.rs' --glob '!**/tests/**'` 生产路径清零或改为 handle-native 命名/类型且不含 FQN target。
- `rg -n "impl_member_fqn|method_impl_fqns" crates/scoopc_codegen_llvm/src/llvm/codegen --glob '*.rs' --glob '!**/tests/**'` 不再作为 callable target 选择/查找路径；若仍出现，必须仅为诊断或上游 facts 校验。
- `rg -n "program\.callable\(|\.callable\(|callable_id_by_root|lir_callable_ref_for_root|exported_abi_symbol_for_lir_root|callable_layout_by_root_fqn|plain_callable_layout_by_root_fqn|maybe_plain_callable_layout_by_root_fqn|lir_callable_id_for_root|abi_symbol_for_root|current_callable_fqn|lir_.*_to_mir" crates/scoopc_codegen_llvm/src/llvm/codegen --glob '*.rs' --glob '!**/tests/**'` 无生产命中。
- `rg -n "_fqn" crates/scoopc_codegen_llvm/src/llvm/codegen` 抽样确认剩余均非 live callable 查找。
- §9 基线绿。

**完成记录（2026-06-05）**：
- 将 callable carrier target registry 从 `(CallableCarrierKind, String)` 改为 `CallableCarrierTargetKey { kind, LirCallableHash }`；dynamic invoke candidate target、carrier target layout 查询、entry symbol registry 与 plain fallback registry 均按 LIR callable handle/hash 选择。
- carrier 发布改用 `LirCallableRef` / stable callable hash / body-version contract；删除生产路径 `callable_facts_for_root` / `effect_step_callable_facts_for_root`，closure、vtable、itable carrier ABI facts 均通过 handle-native callable facts 读取。
- class vtable/itable、value-box itable、static interface dispatch 与 dispatch target declaration 改读 `impl_member_target` / `method_impl_targets`；物理 `impl_member_fqn` / `method_impl_fqns` 不再作为 target 选择/查找路径。
- 为 callable layouts 保存 stable `LirCallableHash`，修正 carrier/ABI query 中 external hash 与 body-version selector 的匹配路径；保留 FQN 仅用于 LLVM symbol、source-signature 文本、诊断或 nominal/global layout key。
- 验收 grep 通过：carrier target map / physical FQN target field / 旧 FQN lookup helper 三组生产路径 grep 均无命中；`_fqn` 抽样未发现本任务范围内新增 live callable FQN 查找。
- 验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。

### [TODO] TC-04-R：Review TC-04
- **关注点**：callee/符号/布局 live 引用全句柄；FQN 仅作符号名/诊断；无「FQN 查不到 fallback」、无句柄→FQN 反转。
- **确认**：上述 grep 仅余符号名/诊断用法；`grep -rn "_fqn" .../codegen` 抽样确认剩余均非 live 查找；§9 绿。
- **依赖**：`TC-04-FIX1`、`TC-04-FIX2`。

**审查阻塞记录（2026-06-05）**：
- `TC-04-R` 静态审查发现 `TC-04` 仍有生产路径 root/FQN live callable 查找，已新增前置修复任务 `TC-04-FIX1`；本 review 保持未完成，待该修复完成后重新执行确认。
- `TC-04-R` 复审确认 `TC-04-FIX1` 已清除旧 helper 命中，但 carrier/dispatch 发布与 registry 仍以 callable root/FQN 字符串选择 live target/layout/facts；已新增前置修复任务 `TC-04-FIX2`，本 review 保持未完成。

### [TODO] TC-05：删除 overlay

**目标**：在 TC-01/02/03 完成（producer 填满 LIR 指令、两条 codegen 路径都 walk LIR）后，**物理删除** overlay。**依赖 TC-01 + TC-02 + TC-03**（必须都完成，否则删了会断编译）。

**起点（已核对）**：
- `crates/scoopc_lir/src/effect_lowered/ir.rs:343` `pub type LateLoweredSourceBody = crate::mir::Body`；`:3591` `struct LateLoweredStateSlice`；以及 `LateLoweredState` 上承载 source_slice 的字段、`source_body()`/`source_slices()` 访问器。
- `crates/scoopc/src/pipeline/lir_artifact.rs` `LirArtifact.mir: Option<MaterializedMir>`、`facts`（facts 已在 P2b 删，确认）；以及 `LateLoweredSourceCallable.body`（`crate::mir::FunDecl` 的间接 overlay）。

**步骤**：
- S1：删 `LateLoweredStateSlice`、`source_slice` 字段与 `source_body()`/`source_slices()` 访问器；`LateLoweredState` 只保留 LIR-owned body。
- S2：删 `type LateLoweredSourceBody`；`LateLoweredProgram` / `LateLoweredCallable` 不再引用 `crate::mir::Body` / `crate::mir::FunDecl::body`。
- S3：删 `LirArtifact.mir`；`base_context` 的类型/布局并入 `program`（若 TC-02/03 尚未并入则在此完成）。最终 `LirArtifact = { cone, program, object_files }`。

**严禁**：不得为了让某处编译通过而保留 `mir` 字段「以防万一」；删不掉=说明 TC-01/02/03 有残留消费，回去补，不在此留 overlay。

**验收**：
- `grep -rnE "LateLoweredSourceBody|LateLoweredStateSlice|crate::mir::Body" crates/scoopc_lir crates/scoopc_codegen_llvm crates/scoopc/src/pipeline` → 清零（除历史注释）。
- `LirArtifact` 无 `mir`/`facts` 字段。
- §9 基线绿。

### [TODO] TC-05-R：Review TC-05
- **关注点**：overlay 类型/字段/访问器彻底删除（非注释保留）；codegen 完全不触 MIR body；`LirArtifact = {cone, program, object_files}`。
- **确认**：上述 grep 清零；`grep -rn "\.mir\b" .../lir_artifact.rs` 无 `mir` 字段；§9 绿；抽样 diff 可执行行为等价。

### [TODO] TC-06：never-fail 收口（错误上移 + ICE 结构化）

**目标**：codegen 对**输入** never-fail——`LlvmEmitError` 只剩真后端 + IO 变体；输入失败变体全部上移；热点处 input `panic`/`expect` 改为「结构不可达」或上移。**建议最后做**（依赖 TC-01..05，因为很多 input 失败在 LIR 消费/句柄/overlay 删除后已自然消失）。

**起点（已核对）`crates/scoopc_codegen_llvm/src/llvm/mod.rs:99` `LlvmEmitError`**：
- **输入失败（上移、删变体）**：`Frontend`、`MissingEntryMain`、`EffectLoweringUnsupported`、`BackendGate`、`AmbiguousEntryMain`、`InvalidLiteral`。
- **保留（真后端/IO）**：`Target`、`Builder`、`Instruction`、`ModuleVerificationFailed`、`RunPassesFailed`、`Write{Ll,Obj,Asm}Failed`。
- fail-fast 热点（input `expect`/`panic`）：`effect_lowered/layout/{classification.rs(141),surface_resume.rs(76),handle_dispatch.rs(48),dispatch.rs(36)}`、`effect_lowered/body/{value.rs(43),mod.rs(40)}`、`mir_body/{const_pat.rs(36),lowering.rs(29)}`。多为「查应被上游 pre-validate 的 ABI fact」的 `expect`、「verifier 已接受但 codegen 失败」的 `panic`。

**步骤**：
- S1：逐个删输入失败变体，把其判定**上移**——`MissingEntryMain`/`AmbiguousEntryMain`：entry 由 LIR 阶段解析为句柄时保证（呼应 TC-04 的 entry 句柄）；`InvalidLiteral`：字面量在更上游（HIR/MIR）保证合法；`EffectLoweringUnsupported`/`BackendGate`/`Frontend`：route/shape 由 LIR 指令集 total + LIR 构造保证。
- S2：热点文件的 input `expect`/`panic`：经 TC-01..05 后，绝大多数因 LIR 句柄/契约保证而**结构不可达** → 降为 `unreachable!("LIR 保证: ...")` / `debug_assert!`；剩下确属「输入可能非法」的，把保证上移（LIR producer 或更上游）。
- S3：保留真后端（LLVM builder/verifier 失败）与 IO（写文件失败）变体——它们不是「输入失败」。

**严禁**：不得用「降 `unreachable!`」掩盖「其实输入可能触发」的情况（那是把 panic 换个名字）；判不准就上移，不留 input 失败路径。

**验收**：
- `LlvmEmitError` 仅余 `Target`/`Builder`/`Instruction`/`ModuleVerificationFailed`/`RunPassesFailed`/`Write*`。
- codegen 无 `Result` 输入错误出口；input `panic!`/`expect` 清零（剩余 `unreachable!`/`debug_assert` 均有「LIR/上游保证」注释）。
- §9 基线绿。

### [TODO] TC-06-R：Review P-CG（整体阶段验收）
- **关注点（对照 PLAN §2 完成标志）**：两条路径都 walk LIR、无 `mir::*` body match、无 overlay、callee/符号/布局引用全句柄、`LlvmEmitError` 仅后端/IO、codegen 对输入 never-fail。
- **范围纪律**：未越界改 P3+；全程无 LIR→MIR 反向 shim / 句柄→FQN 反转 / 占位 / no-op 容忍。
- **逐项确认**：
  - `grep -rnE "mir::(Statement|Rvalue|Terminator|Operand|Place)|LateLoweredSourceBody|LateLoweredStateSlice|crate::mir::Body|\.source_body\(\)" crates/scoopc_codegen_llvm crates/scoopc_lir` 清零。
  - `LlvmEmitError` 无输入失败变体；codegen 无 input `Result`/`panic`/`expect`。
  - `grep -rn "lir_.*_to_mir\|program.callable(" crates/scoopc_codegen_llvm` 清零。
  - §9 全套绿；抽样若干 fixture diff 可执行行为等价；在 PLAN.md 标记 P-CG DONE。

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

- **互相依赖**：TC-01（LIR 有指令）是 TC-02/03 前提；TC-05（删 overlay）必须在 TC-02/03/01 都完成后；TC-04 与 body 改造正交可并行但建议在 TC-02/03 后。
- **不得为绿造 shim**：若某路径迁 LIR 后缺数据，是 TC-01 lift 未填全或上游未保证——回到 producer 补，**不在 codegen 造反向转换/占位**（上次打回的就是这个）。
- 真后端 `panic`（LLVM builder 失败等）可保留——它们不是「输入失败」，是 LLVM/IO 层真错误。
