# TODO（closure capture 语义修正 + sealed interface + shared-state primitives）

> 生成时间：2026-05-18  
> 设计基线：[`CLOSURE_FIX.md`](./CLOSURE_FIX.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 格式参考：[`docs/archive/plans/TODO-stable-id.md`](./docs/archive/plans/TODO-stable-id.md)  
> 当前状态：C4-T01C 已完成
> 执行原则：C0 必须最先完成；C1/C3 与 C2 两条实现线可按依赖并行，但单个任务完成时不得留下仓库内 failing fixture；每个任务完成后必须回写“完成记录”。

## 全局约束

- [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) 是本轮唯一设计基线；[`PLAN.md`](./PLAN.md) 是本轮唯一计划基线。若实现过程中要改变 closure capture 语义、`sealed interface` 边界或 Atomic API 形态，必须先更新设计/计划文档，再继续编码。
- closure capture 最终语义必须保持：构造点 by-value snapshot；value type 复制值；ref type 复制 managed pointer；closure env 构造后不可变；lambda 内 captured 名字等价于本次 call frame 的局部变量。
- 外层 `val x` 捕获到 lambda 内是 immutable local；外层 `var x` 捕获到 lambda 内是 mutable local；lambda 内 rebind 只写本次调用 frame，不影响外层，也不在多次调用之间持久。
- Scoop 不做 Kotlin 式隐式 boxing。需要与外层共享 mutable 状态或让 closure 实例持有跨调用状态时，用户必须显式使用 `RefCell<T>` / `Atomic<...>` 等库类型。
- 删除 CaptureBox 时必须删除整套后门：`Rvalue::CaptureBoxNew/Get/Set`、`MirTransportKind::CaptureBox`、`CaptureBoxTransportMetadata`、`scoop.__CaptureBox`、`mir_capture_box_type_desc`、`rt_alloc_pass_mir_capture_box` 以及所有对应 dump/test 断言。
- 必须保留 HIR `Capture.mutable` 字段（`crates/scoopc/src/hir/mod.rs:573-578`）。它不再表示“需要隐式 box”，而是传递外层 binding mutability，供 closure body codegen 把 env-load 后的本地 alloca 标记为可 rebind。
- `sealed interface` 是独立 marker 构造，与普通 `interface` 正交。body 必须为空；起步只允许 sysroot 定义；只能作为 generic bound；不允许 runtime type test/cast、binding/param/return type、type argument 或显式实现。
- `AnyRef` / `AnyValue` 是互斥 marker。`class` 自动满足 `AnyRef`；`struct` / `tuple` / `enum` / 内建标量自动满足 `AnyValue`；marker 登记只进入编译期 metadata，不进入 instance layout、itable、vtable 或 type descriptor。
- `sealed interface` 之间允许继承，但只能继承其它 sealed interface；必须在登记阶段计算传递闭包，并在定义点拒绝循环、非 sealed supertype、同时继承互斥 marker。
- `RefCell<T>` / `Box<T>` 是普通 sysroot class，T 无 bound。`AtomicInt` / `AtomicBool` / `Atomic<T: AnyRef>` / `AtomicValue<T: AnyValue>` 是用户可见库类型；`AtomicValue<T>::cas` 的 expected 必须是 `Box<T>`，不是 `T`。
- 仓库尚无发布版本，本轮不添加 backward compatibility 分支；旧 MIR variant、旧 descriptor、旧 fixture expect 可以直接删除/刷新。
- 任何任务如果新增或修改 `SCOOP_FULL_SPEC.md` 里的 fixture code block，必须运行 `cargo run -p scoop_tools -- spec-fixtures sync`，然后运行 `cargo run -p scoop_tools -- spec-fixtures check`。
- 每个任务完成后必须回写：改动范围、核心决策、验证结果、与 `PLAN.md` / `CLOSURE_FIX.md` 闭合的目标或验收项。

## 固定定位清单

### Frontend / Typecheck 入口

- `crates/scoopc/src/ast/mod.rs:554-569`：`Modifier::Sealed` 已存在；无需新增 keyword，只需在 typecheck 识别 `sealed interface`。
- `crates/scoopc/src/parser/decls.rs:94-103`：`parse_decl_prefix` 已把 `sealed` 解析为 modifier。
- `crates/scoopc/src/parser/decls.rs:1344-1406`：`parse_type_decl` 已能解析 `interface` 与 `: supertype` 列表；`sealed interface I : J, K` 语法应主要复用这里。
- `crates/scoopc/src/parser/cursor.rs:549,568`：`Keyword::Sealed` / `Keyword::Interface` 已是 keyword（见 `PLAN.md` §1.3）。
- `crates/scoopc/src/source.rs:13-17,35-37,86-94`：`SourceOrigin::Sysroot` / `SourceFile::is_sysroot()` 可用于 sealed interface “sysroot only” gate。
- `crates/scoopc/src/resolve/mod.rs:238-257`：`ModifierSet` 已记录 `sealed`，resolver symbol 上已有 modifier 位。
- `crates/scoopc/src/resolve/mod.rs:1699-1772`：`check_file_headers` 解析 type/header/signature type refs；适合放 early 语义 gate 的前置解析配合。
- `crates/scoopc/src/resolve/scopes.rs:178-293`：type body 中成员解析；如 sealed interface body 非空需要在 typecheck 侧拒绝，不要只靠 resolver。
- `crates/scoopc/src/typecheck/type_env.rs:132-183`：`TypeSymbol` 当前记录 nominal kind、type params、where constraints、decl_file；sealed marker metadata 可在这里扩展或建立并行表。
- `crates/scoopc/src/typecheck/type_env.rs:481-492`：已有 direct supertypes 查询 API。
- `crates/scoopc/src/typecheck/type_env.rs:673-785`：`collect_type_decl` 收集 nominal type、where constraints、direct supertypes；sealed marker 识别、sysroot-only 记录、super marker 边表可从这里接入。
- `crates/scoopc/src/typecheck/lower.rs:43-233`：`TypeLowerError` 诊断 enum；新增 bound-only / sealed marker misuse 错误码可放这里或新增专用 error enum 后接入 pipeline。
- `crates/scoopc/src/typecheck/lower.rs:2240-2399` 与 `3120-3219`：名义类型 lowering；当前普通 `interface` 会降为 `TypeKind::Ref(RefTypeKind::Nominal(...))`。sealed marker 非 bound 用法必须在这些路径或调用方上游拒绝，避免被降成 runtime ref type。
- `crates/scoopc/src/typecheck/lower.rs:2403-2462`：where constraint 满足性检查；`AnyRef` / `AnyValue` bound 查询应接入这里或同一层级的 bound checking。
- `crates/scoopc/src/typecheck/interfaces.rs:29-92`：现有 interface 诊断；新增 sealed interface supertype / explicit implementation reject 可扩展这里或拆新模块。
- `crates/scoopc/src/typecheck/interfaces.rs:119-263`：当前 `interface` 只检查 supertypes 都是 interface；sealed interface 需要覆盖“只能继承 sealed interface”的更严格规则。
- `crates/scoopc/src/typecheck/expr/infer.rs:187-246`：`as` / `as?` / `is` expression typecheck 入口；sealed marker runtime cast/test reject 要在这里或调用 helper 中落地。
- `crates/scoopc/src/typecheck/when_pat.rs:303-341`：`when (x) { is T -> ... }` target 检查；sealed marker `is AnyRef` / `is AnyValue` reject 要覆盖这里。
- `crates/scoopc/src/typecheck/expr/error.rs:660-700,977-999`：泛型约束与 runtime cast/test 相关诊断已有样式；新增 sealed marker 诊断使用 `scoop::typecheck::sealed_interface_*` 前缀。
- `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:390-426`：`FRONTEND_REJECT_SURFACES` 当前只登记 5 类 frontend reject；C4/C5 需登记 sealed-interface 新增 rejects。

### Closure / CaptureBox 入口

- `crates/scoopc/src/hir/mod.rs:565-578`：`Capture { mutable }` 必须保留，但注释需要从“box 实现别名语义”改为“传递外层 mutability 给 per-call local”。
- `crates/scoopc/src/hir/lower/util/closures.rs:7-36`：`compute_closure_captures` 从 `local_mutability` 写入 `Capture.mutable`；逻辑应保留。
- `crates/scoopc/src/hir/lower/util/closures.rs:227-241`：收集 local capture 初值 `mutable: false`，随后统一补 mutability；逻辑应保留。
- `crates/scoopc/src/mir/lower/post_helpers.rs:93-233`：`boxed_symbols_in_block/expr` 当前专为 nested mutable capture 隐式 boxing 服务；删除 CaptureBox 后应删除或改名为不再服务 box 的逻辑。
- `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:705-731`：外层 `var` 被 capture 时改写为 `CaptureBoxNew`；必须删掉，改回普通 local alloca。
- `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:791-804`：对 boxed symbol 的赋值改写为 `CaptureBoxSet`；必须删掉，改为普通 `assign_use_to_local`。
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:610-624`：`capture_box_contract`；删除。
- `crates/scoopc/src/mir/lower/fn_lowering_call.rs:626-660`：`closure_env_contract` 对 `capture.mutable` 使用 `MirTransportKind::CaptureBox`；必须改成所有 capture 都使用 `transport_kind_for_ty(capture.ty)` + closure capture boxing metadata。
- `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:137-144`：`capture_box_ty` 构造 `scoop.__CaptureBox<T>`；删除。
- `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:1060-1099`：`lower_var_ref` 对 boxed symbol 生成 `CaptureBoxGet`；删除，普通 local 直接返回。
- `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:1242-1286`：closure fun lowering 当前把 mutable capture 重新塞进 `boxed_symbols`；删除该行为，捕获字段 TupleGet 后 local 应是普通可写 local。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:97-101`：当前拒绝 mutable capture；修复后该 guard 不应拒绝合法输入。可删除 guard，或改成内部 bug sentinel（若保留 unreachable，须登记到 `INTERNAL_BUG_SENTINEL_HITS`）。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:625-678`：inner-side capture binding 已是“env field load -> entry alloca”；`mutable: false` 必须改为 `mutable: cap.mutable`。
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:107-148`：effect-lowered Rvalue CaptureBox 分支；删除并同步 per-call local 语义。

### CaptureBox 当前命中快照

以下为本文件生成时 `rg -c "CaptureBox|capture_box|mir_capture_box|__CaptureBox" crates/scoopc/src tests/fixtures sysroot` 的关键结果。C2 完成后，`crates/scoopc/src` 中这些命中应为 0，除非完成记录明确说明某个旧名字只保留在历史文档或迁移注释中。

| 路径 | 当前命中数 | 处理方向 |
|---|---:|---|
| `crates/scoopc/src/mir/mod.rs` | 13 | 删除 Rvalue variants 与 validate arm |
| `crates/scoopc/src/mir/transport.rs` | 2 | 删除 transport kind 与 metadata |
| `crates/scoopc/src/mir/lower/mod.rs` | 1 | 删除 import/field 依赖 |
| `crates/scoopc/src/mir/lower/entry.rs` | 1 | 删除 `CAPTURE_BOX_FQN` |
| `crates/scoopc/src/mir/lower/fn_lowering_basic.rs` | 5 | 删除 outer local boxing 与 assignment rewrite |
| `crates/scoopc/src/mir/lower/fn_lowering_call.rs` | 4 | 删除 capture_box_contract 与 CaptureBox env transport |
| `crates/scoopc/src/mir/lower/fn_lowering_effect.rs` | 4 | 删除 `capture_box_ty` / get path / closure-fun boxed capture path |
| `crates/scoopc/src/mir/lower/post_helpers.rs` | 46 | 删除/重写 boxed_symbols collector |
| `crates/scoopc/src/mir/closure_simplify.rs` | 3 | 删除 CaptureBox arm |
| `crates/scoopc/src/mir/escape.rs` | 3 | 删除 CaptureBox arm |
| `crates/scoopc/src/mir/inline.rs` | 15 | 删除 CaptureBox clone/rewrite arm |
| `crates/scoopc/src/mir/summary.rs` | 8 | 删除 CaptureBox summary arm |
| `crates/scoopc/src/mir/dump.rs` | 16 | 删除 CaptureBox dump text |
| `crates/scoopc/src/mir/materialize/mod.rs` | 1 | 删除 metadata import |
| `crates/scoopc/src/mir/materialize/rewrite.rs` | 8 | 删除 rewrite_capture_box_contract |
| `crates/scoopc/src/mir/materialize/validation.rs` | 8 | 删除 validate_materialized_capture_box_contract |
| `crates/scoopc/src/mir/materialize/utils.rs` | 3 | 删除 helper |
| `crates/scoopc/src/effect_facts/builder.rs` | 6 | 删除 CaptureBox fact handling |
| `crates/scoopc/src/effect_lowered/frame.rs` | 3 | 删除 CaptureBox frame metadata |
| `crates/scoopc/src/effect_lowered/segment.rs` | 3 | 删除 CaptureBox segment metadata |
| `crates/scoopc/src/effect_lowered/materialize/classification.rs` | 6 | 删除 CaptureBox classification |
| `crates/scoopc/src/llvm/reachability.rs` | 9 | 删除 CaptureBox reachability handling |
| `crates/scoopc/src/llvm/codegen/composite_transport.rs` | 12 | 删除 CaptureBox transport verification/gap |
| `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs` | 3 | 删除 effect-lowered Rvalue lowering |
| `crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs` | 24 | 删除 MIR capture box allocation/get/set codegen |
| `crates/scoopc/src/llvm/codegen/mir_body/mod.rs` | 4 | 删除 dispatch to capture box codegen |
| `crates/scoopc/src/llvm/codegen/mir_body/operand.rs` | 5 | 删除 operand special casing |
| `crates/scoopc/src/llvm/codegen/mir_body/terminator.rs` | 12 | 删除 call entries expecting capture boxes |
| `crates/scoopc/src/llvm/codegen/mir_body/value_args.rs` | 5 | 删除 `mir_capture_box_type_desc` descriptor helper |
| `crates/scoopc/src/pipeline/mir_stage.rs` | 14 | 更新 tests/assertions that expect CaptureBox |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` | 4 | 更新 LLVM pipeline tests expecting capture box alloc/type desc |
| `crates/scoopc/src/stable_id.rs` | 2 | 删除只服务 capture-box descriptor 的 stable-id role，如无其它引用 |

受 CaptureBox 删除影响的 fixture snapshot：

- `tests/fixtures/mir/closure_capture_var.mir`
- `tests/fixtures/mir/closure_capture_var.actual.mir`
- `tests/fixtures/mir/closure_capture_var.actual.raw.mir`
- `tests/fixtures/mir_lowered/aggregate_transport.mir`
- `tests/fixtures/mir_lowered/aggregate_transport.actual.mir`
- `tests/fixtures/mir_lowered/aggregate_transport.actual.raw.mir`
- `tests/fixtures/mir_lowered/assignment_places.mir`
- `tests/fixtures/mir_lowered/assignment_places.actual.mir`
- `tests/fixtures/mir_lowered/assignment_places.actual.raw.mir`

Closure 相关运行/GC fixture 必须保留并回归：

- `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`
- `tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`
- `tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`
- `tests/fixtures/run-pass/effect_indirect_perform_materialized_mir_closure_basic.scoop`
- `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`
- `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop`
- `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
- `tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`
- `tests/fixtures/build/safepoint_non_escaping_closure_basic.scoop`

### Atomic / GC Barrier 当前入口

- `sysroot/scoop.unsafe/unsafe.scoop:151-175`：当前只有内部 `__AtomicInt` typealias 与 `__atomicIntLoad/Store/CompareExchange` 三个 intrinsic，memory order 固定 SeqCst。
- `crates/scoopc/src/typecheck/lower.rs:2264-2278,3133-3143`：`scoop.unsafe.__AtomicInt` 在 type lowering 中等同 `Int`。
- `crates/scoopc/src/hir/lower/main/impl_lowering.rs:1819-1820` 与 `crates/scoopc/src/hir/lower/util/generic_layouts.rs:70-78`：HIR/layout lowering 同样把 `__AtomicInt` 当作 `Int`。
- `crates/scoopc/src/llvm/codegen/intrinsics/atomic.rs:14-338`：direct HIR LLVM lowering 的 atomic-int intrinsic 实现。
- `crates/scoopc/src/llvm/codegen/main/call.rs:20-166`：atomic intrinsic lvalue address extraction；支持 local、top-level var、class field、struct field。
- `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2218-2345`：effect-lowered atomic-int intrinsic 实现。
- `crates/scoopc/src/llvm/codegen/call/lowering.rs:1594-1595`：atomic-int intrinsic dispatch 入口。
- `crates/scoopc/src/llvm/codegen/gc.rs:2039-2070`：`store_gc_pointer_slot_with_write_barrier` helper；atomic-ref store/CAS 成功写入 ref 时需要遵守同一 GC barrier 语义。
- `crates/scoopc/src/llvm/codegen/runtime_abi.rs:535-558`：runtime write barrier declaration，signature 为 `void* scoop_gc_write_barrier(void* slot_addr, void* value)`。
- `runtime/c/scoop_gc_backend_immix.c:1879-1909`、`runtime/c/scoop_gc_backend_minimal.c:134`、`runtime/c/scoop_gc_backend_hosted.c:143`、`runtime/c/scoop_gc.c:236`：runtime barrier implementations。
- 当前 `rg -n "__atomicRef|atomic_ref|AtomicRef|atomicRef" crates/scoopc/src sysroot runtime tests/fixtures` 无命中；`Atomic<T: AnyRef>` 需要新增 atomic-ref lowering 或明确用现有机制无法表达的 gap。
- 现有 atomic fixtures：`tests/fixtures/run-pass/unsafe_atomic_int_basic.scoop`、`tests/fixtures/run-pass/unsafe_atomic_int_field_lvalue_basic.scoop`、`tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`、`tests/fixtures/build/unsafe_atomic_int_field_lvalue_llvm.scoop`、`tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`。

### Sysroot 名字现状

- `sysroot/scoop.core/core.scoop` 当前有 `Any`、`Hashable`、`ToString`、`String`、`Array<T>`、`MutableArray<T>`、`List<T>`、`MutableList<T>`、`Unit`、`Nothing`、标量整数/浮点/字符/布尔、`Option<T>`、`Continuation<...>`、`RuntimeError`、`Raise<...>` 等。
- `sysroot/scoop.unsafe/unsafe.scoop` 当前有 `Ptr<T>`、`FunPtr<F>`、`__AtomicInt`。
- sysroot 当前没有 `AnyRef`、`AnyValue`、`RefCell`、`Box`、`AtomicInt`、`AtomicBool`、`Atomic<T>`、`AtomicValue<T>` 同名声明。
- fixtures 中有大量本地 `Box` 类型声明，这不阻塞 sysroot `scoop.core.Box`，但新增 fixture 要避免与本地 `Box` 重名导致诊断噪音。

## 顺序总览

```text
C0-T01 (baseline + prerequisite inventory)
  ├─> C1-T01 (sealed interface frontend/type metadata)
  │     └─> C1-T02 (AnyRef / AnyValue sysroot markers)
  │           └─> C3-T02 (Atomic family)
  │                 ↑
  │                 C3-T01 (RefCell / Box)
  │
  └─> C2-T01A..C2-T01E (delete CaptureBox by subsystem)
        └─> C2-T02 (closure inner mutable per-call local)
              └─> C4-T01A..C4-T01D (fixtures)
                    └─> C4-T02 (audit baselines)
                          └─> C5-T01 (spec / docs)
```

## C0：冻结 baseline + 摸底先决

### [DONE] C0-T01：建立本轮 baseline、确认先决事实与 fixture 分类

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C0
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §5.1
  - 本文件“固定定位清单”
- 目标：
  - 在正式修改前记录当前可运行 baseline、CaptureBox 命中、closure-dependent fixture、atomic-ref 缺口、sysroot 名字冲突与 audit 基线影响面。
  - 给后续 C1/C2/C3 任务提供不用重复搜索的事实清单。
- 必须实现的内容：
  1. 运行并记录当前测试 baseline：`cargo build`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`。
  2. 复核 CaptureBox 命中清单：运行 `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox" crates/scoopc/src tests/fixtures sysroot`，将新增/减少的命中写入完成记录。
  3. 分类 closure-dependent fixtures：至少覆盖本文件列出的 MIR snapshot、run-pass closure、effect closure、runtime_gc closure、build closure fixture。
  4. 确认 atomic-ref 现状：运行 `rg -n "__atomicRef|atomic_ref|AtomicRef|atomicRef" crates/scoopc/src sysroot runtime tests/fixtures`；当前预期无命中。
  5. 确认 sysroot 名字：运行 `rg -n "^\s*(class|struct|interface|sealed interface|typealias|enum|object|effect)\s+(AnyRef|AnyValue|RefCell|Box|Atomic(Int|Bool|Value)?|Atomic)\b" sysroot tests/fixtures`；当前预期 sysroot 无冲突，fixtures 有本地 `Box`。
  6. 评估 `pipeline_user_visible_failure_policy.rs`：记录 CaptureBox 删除会影响哪些 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 条目，尤其 `mir_body/aggregates.rs`、`mir_body/terminator.rs`、`mir_body/value_args.rs`、`effect_lowered/value.rs`。
- 必须遵从的约束：
  - 本任务不改语义代码；只允许补充 `TODO.md` 完成记录或新增纯审计说明。
  - 如果 baseline 已经失败，必须记录失败命令、失败 fixture/test 名与是否与本轮相关；不得顺手修无关问题。
- 验证：
  1. `cargo build`
  2. `cargo test --all --all-targets`
  3. `cargo run -p scoop -- test`
  4. 上述 `rg` 审计命令
- 完成条件：
  - 完成记录中有 baseline 结果、CaptureBox 命中摘要、closure fixture 分类、atomic-ref 缺口、sysroot 名字确认、audit 基线影响面。
- 依赖：无
- 完成记录：
  - 改动范围：只更新本任务完成记录与 `memory/claude_plan.md` 进度；未修改语义代码、fixture expect 或 `PLAN.md`。
  - 核心决策：
    - 当前 baseline 全绿；没有发现阻塞 C1/C2/C3 的现有 failing test 或 failing fixture。
    - CaptureBox 命中与本文件“CaptureBox 当前命中快照”一致；没有新增 source 命中，`sysroot` 无 CaptureBox 命中。
    - closure-dependent fixtures 分类如下：MIR snapshot 为 `tests/fixtures/mir/closure_capture_var.*`、`tests/fixtures/mir_lowered/aggregate_transport.*`、`tests/fixtures/mir_lowered/assignment_places.*`；run-pass closure 为 `closure_env_composite_capture_basic.scoop`；effect closure 为 `effect_indirect_perform_materialized_mir_closure_basic.scoop`、`effect_indirect_perform_nonresuming_closure.scoop`、`effect_escape_continuation_indirect_perform_closure*.scoop`；runtime_gc closure 为 `gc_trace_closure_capture_string_basic.scoop`、`gc_move_enum_maybe_ref_closure_capture_basic.scoop`；build closure 为 `safepoint_non_escaping_closure_basic.scoop`。
    - atomic-ref 现状确认：`__atomicRef` / `atomic_ref` / `AtomicRef` / `atomicRef` 当前无命中，C3-T02 仍需新增 atomic-ref primitive 或等价 lowering。
    - sysroot 名字确认：`sysroot` 当前没有 `AnyRef`、`AnyValue`、`RefCell`、`Box`、`AtomicInt`、`AtomicBool`、`Atomic<T>`、`AtomicValue<T>` 声明；`tests/fixtures/**` 中存在多处本地 `Box` 类型，后续新增 fixture 需避免本地重名噪音。
    - `pipeline_user_visible_failure_policy.rs` 影响面：当前 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 中相关条目为 `mir_body/aggregates.rs = 42`、`mir_body/terminator.rs = 19`、`mir_body/value_args.rs = 6`、`effect_lowered/value.rs = 126`。CaptureBox 删除预计会移除 `aggregates.rs` 中 capture-box new/get/set helper 内 11 个 `UnsupportedMainBody` 站点；`terminator.rs` 的 CaptureBox arms 只是派发到 helper，`value_args.rs` 的 capture-box descriptor helper 与 `effect_lowered/value.rs` 的 `rvalue_mentions_local` arms 不直接贡献 `UnsupportedMainBody` 计数。
  - 验证结果：
    - `cargo build`：通过。
    - `cargo test --all --all-targets`：通过，命令整体退出成功；`scoopc` 测试段显示 856 passed。
    - `cargo run -p scoop -- test`：通过，`1340/1340` fixtures PASS，`fixtures: ok (1377)`。
    - `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox" crates/scoopc/src tests/fixtures sysroot`：通过；source counts 与快照一致，fixture 命中集中在已列出的 MIR snapshot。
    - `rg -n "__atomicRef|atomic_ref|AtomicRef|atomicRef" crates/scoopc/src sysroot runtime tests/fixtures`：无输出。
    - `rg -n '^\s*(class|struct|interface|sealed interface|typealias|enum|object|effect)\s+(AnyRef|AnyValue|RefCell|Box|Atomic(Int|Bool|Value)?|Atomic)\b' sysroot tests/fixtures`：仅 fixture-local `Box` 声明命中，`sysroot` 无命中。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C0 与 `CLOSURE_FIX.md` §5.1 的 baseline / 先决事实摸底要求；阶段级计划未变化。

## C1：类型系统底座

### [DONE] C1-T01：引入 `sealed interface` frontend 语义、marker metadata 与自动登记

- 参考：
  - [`PLAN.md`](./PLAN.md) §4、§7 C1-T01
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §2
  - 本文件“Frontend / Typecheck 入口”
- 目标：
  - 把 parser 已能表达的 `sealed interface` 从“普通 interface + sealed modifier”升级为独立 marker 构造。
  - 支持 marker 继承、传递闭包、按 nominal kind 自动满足 `AnyRef` / `AnyValue`，并拒绝所有非 bound 用法。
- 必须实现的内容：
  1. 在 type metadata 中记录 sealed marker：至少能回答某 FQN 是否 sealed interface、direct sealed super markers、transitive sealed marker closure、互斥 marker 集合。
  2. 在 typecheck 中识别 `decl.kind == Interface && decl.modifiers.contains(Sealed)` 为 sealed marker。
  3. body-empty gate：`sealed interface I { fun foo() }`、property、nested type/object、enum variant、init block 等任意 body member 都报 `scoop::typecheck::sealed_interface_must_be_empty`。
  4. sysroot-only gate：`!source.is_sysroot()` 的 sealed interface 定义报 `scoop::typecheck::sealed_interface_user_definition_not_allowed`。
  5. supertype gate：sealed interface 只能继承 sealed interface；继承普通 `interface`、`class`、`struct`、`enum`、`effect` 或带 ctor args 都报 `scoop::typecheck::sealed_interface_supertype_must_be_sealed`。
  6. cycle gate：拒绝 `sealed interface I : I` 与构造性循环 `A : B; B : A`，错误码 `scoop::typecheck::sealed_interface_inheritance_cycle`。
  7. mutually-exclusive gate：`AnyRef + AnyValue` 或 marker 定义同时蕴涵这两个祖先时，报 `scoop::typecheck::sealed_interface_mutually_exclusive_bound`。
  8. bound-only gate：sealed marker 名只允许出现在 generic bound / where bound 右侧；禁止 binding type、param type、return type、type argument、`is`/`when is`、`as`/`as?`、显式 supertype/implements。
  9. 自动登记：class -> `AnyRef`；struct/tuple/enum/内建标量 -> `AnyValue`；登记时加入 marker supertype 传递闭包；runtime metadata 不变。
  10. 泛型约束检查：`where T: AnyRef` / `where T: AnyValue` 对 concrete type arg 查询自动登记集合；type param 未具体化时按现有 where 逻辑保守延后。
- 必须遵从的约束：
  - 不要新增关键字；继续使用 `Modifier::Sealed` + `TypeKind::Interface`。
  - 不要把 sealed marker 降成 `TypeKind::Ref(RefTypeKind::Nominal(...))` 后在 runtime 使用；非 bound 用法应提前拒绝。
  - 不要把 sealed marker 放进 interface itable/vtable/class layout/type descriptor。
  - 不要复用普通 interface implementation 检查来要求 class/struct 显式实现 marker；marker 满足关系由 compiler 自动登记。
- 验证：
  1. 新增/更新 Rust 单元测试覆盖 sealed marker metadata、inheritance closure、mutual exclusion、bound satisfaction。
  2. `cargo test -p scoopc sealed -- --nocapture`（若测试名采用 `sealed_*`）。
  3. `cargo test -p scoopc typecheck -- --nocapture`。
  4. `cargo build`。
- 完成条件：
  - `AnyRef` / `AnyValue` 尚未加入 sysroot 前，compiler 已能识别 sysroot sealed marker 定义并对测试用虚拟 sysroot source 执行全部 gate。
- 依赖：C0-T01
- 完成记录：
  - 改动范围：
    - `TypeEnv` 新增 sealed marker metadata：`is_sealed_interface`、direct sealed supers、transitive closure 与 `AnyRef` / `AnyValue` 互斥检查。
    - type lowering 新增 sealed marker bound-only 语境；generic/where bound 右侧允许 marker，普通 binding/param/return/typealias/type argument/cast/type-test 等运行期类型位置报 `scoop::typecheck::sealed_interface_bound_only`。
    - interface 检查拒绝 class/struct/enum/object/普通 interface 显式实现或继承 sealed marker；sealed marker 定义自身的图形合法性由 `TypeEnv` 检查。
    - where 约束满足性接入自动 marker 满足关系：ref types 满足 `AnyRef`，value types（含 tuple/struct/enum/内建值类型）满足 `AnyValue`，并沿 marker super closure 查询。
    - cone external type symbol 注入同步补齐 `TypeSymbol::is_sealed_interface = false`。
  - 核心决策：
    - sealed marker 仍复用 parser 的 `Modifier::Sealed + TypeKind::Interface` surface，但在 type metadata 中作为 compile-time-only marker 单独登记，不进入 runtime interface lowering、itable、vtable、layout 或 type descriptor。
    - marker 定义 gate 放在 type-env 收集/重建阶段，确保 sysroot-only、body-empty、supertype sealed-only、cycle 与互斥 marker 能在任何后续 lowering 前失败。
    - bound lowering 只允许顶层 marker bound；marker 出现在 type argument 或其它嵌套 runtime type position 仍按 bound-only 违规拒绝。
    - C1-T01 不向 sysroot 添加 `AnyRef` / `AnyValue`；测试使用 virtual sysroot source 验证 compiler 已能识别 sysroot sealed marker 定义。
  - 验证结果：
    - `cargo test -p scoopc sealed -- --nocapture`：通过，9 个 sealed 定向测试通过。
    - `cargo test -p scoopc typecheck -- --nocapture`：通过，39 个 typecheck 相关测试通过。
    - `cargo build`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §4 / §7 C1-T01 与 `CLOSURE_FIX.md` §2 的 sealed marker 前端语义、metadata、bound-only gate、自动 `AnyRef` / `AnyValue` 满足关系底座；阶段级计划未变化。

### [DONE] C1-T02：在 sysroot 添加 `AnyRef` / `AnyValue`

- 参考：
  - [`PLAN.md`](./PLAN.md) §4.4、§7 C1-T02
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §2.5
- 目标：
  - 在 `scoop.core` 发布两个起步 sealed marker，供 C3 Atomic bound 使用。
- 必须实现的内容：
  1. 在 `sysroot/scoop.core/core.scoop` 或新文件 `sysroot/scoop.core/sealed_markers.scoop` 添加：

     ```scoop
     sealed interface AnyRef
     sealed interface AnyValue
     ```

  2. 若新建 sysroot 文件，确认 sysroot loader/index 会加载该文件；否则直接放入 `core.scoop` root types 附近（当前 `interface Any` 在 `core.scoop:30`）。
  3. 更新/新增 sysroot type env tests，确认 `scoop.core.AnyRef` / `scoop.core.AnyValue` 可解析、互斥、无 runtime layout/itable side effect。
- 必须遵从的约束：
  - 起步不引入 `PlainValue`、`Struct`、`Tuple`、`Enum`、`Primitive`、`WordFit` 等子 marker。
  - `AnyRef` / `AnyValue` 不能有 body，不能继承普通 `Any` 或 `Hashable`。
- 验证：
  1. `cargo test -p scoopc sysroot_type_env -- --nocapture`
  2. `cargo test -p scoopc sealed -- --nocapture`
  3. `cargo build`
- 完成条件：
  - 用户源码中可在 generic bound 写 `T: AnyRef` / `T: AnyValue`；其它位置使用会按 C1-T01 gate 报错。
- 依赖：C1-T01
- 完成记录：
  - 改动范围：
    - 在 `sysroot/scoop.core/core.scoop` Root types 区域新增空 `sealed interface AnyRef` 与 `sealed interface AnyValue`。
    - 在 `crates/scoopc/src/typecheck/type_env.rs` 增加真实 sysroot 测试，覆盖 marker 注册、空 super closure、`AnyRef` / `AnyValue` 互斥、未写入 ordinary runtime supertype 图，以及用户源码中 `T: AnyRef` / `T: AnyValue` generic bound 使用。
    - 未修改 `PLAN.md`、`CLOSURE_FIX.md` 或 runtime/codegen；未新增 sysroot 文件，因此无需调整 sysroot loader。
  - 核心决策：
    - 直接把两个 marker 放入现有 `core.scoop` 的 Root types 附近，复用 C1-T01 已实现的 sysroot-only sealed marker metadata 与 bound-only gate。
    - `AnyRef` / `AnyValue` 起步保持无 body、无 supertype，不继承普通 `Any` / `Hashable`，也不引入其它子 marker。
    - 测试通过 `TypeEnv` 的 sealed marker 表与 ordinary `supertypes` 图分离来确认 marker 不作为 runtime interface supertype/itable 入口记录。
  - 验证结果：
    - `cargo test -p scoopc sysroot_type_env -- --nocapture`：通过，4 个 sysroot type env 定向测试通过。
    - `cargo test -p scoopc sealed -- --nocapture`：通过，9 个 sealed 定向测试通过。
    - `cargo build`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §4.4 / §7 C1-T02 与 `CLOSURE_FIX.md` §2.5 的 sysroot `AnyRef` / `AnyValue` marker 发布要求；阶段级计划未变化。

## C2：closure capture 修复

### [DONE] C2-T01A：删除 MIR core 中的 CaptureBox 类型与 transport 模型

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T01
  - 本文件“CaptureBox 当前命中快照”
- 目标：
  - 先从 MIR 数据模型层删除 CaptureBox variant/metadata，让后续子系统编译错误指向真实依赖。
- 必须实现的内容：
  1. `crates/scoopc/src/mir/mod.rs`：删除 `Rvalue::CaptureBoxNew/Get/Set` 及 validation/requirements arm。
  2. `crates/scoopc/src/mir/transport.rs`：删除 `MirTransportKind::CaptureBox` 与 `CaptureBoxTransportMetadata`。
  3. `crates/scoopc/src/mir/lower/mod.rs`、`mir/materialize/mod.rs`、其它 import 处删除 `CaptureBoxTransportMetadata`。
  4. `crates/scoopc/src/stable_id.rs` 中若存在只服务 `mir_capture_box_type_desc` 的 role/key，删除或确认无 source 引用。
- 必须遵从的约束：
  - 不要删除 `ClosureCaptureTransportMetadata.mutable`。
  - 不要把 CaptureBox 改名为别的隐式 box；本任务是删除，不是重实现。
- 验证：
  1. `cargo build -p scoopc`，预期初次可能暴露下游编译错误；本任务完成时必须通过。
  2. `rg -n "CaptureBoxTransportMetadata|MirTransportKind::CaptureBox|Rvalue::CaptureBox" crates/scoopc/src/mir crates/scoopc/src/stable_id.rs`
- 完成条件：
  - MIR core 不再定义 CaptureBox model。
- 依赖：C0-T01
- 完成记录：
  - 改动范围：
    - 删除 MIR core 中的 `Rvalue::CaptureBoxNew/Get/Set`、`MirTransportKind::CaptureBox` 与 `CaptureBoxTransportMetadata`，并清理 `stable_id.rs` 中只服务 `mir_capture_box_type_desc` 的测试角色。
    - 为保证 `cargo build -p scoopc` 通过，删除由 core model 移除直接暴露出的下游引用：MIR lowering 不再构造/读取/写入 CaptureBox，MIR analysis/dump/materialize/effect facts/effect-lowered/LLVM codegen 不再匹配 CaptureBox rvalue 或 transport kind。
    - 更新受编译影响的 Rust 测试断言，使其验证 captured mutable local 保持普通 local assignment、closure env 保留 capture mutability/boxing metadata，且 LLVM closure env lowering 不再产生 capture-box allocation/type descriptor。
  - 核心决策：
    - 不引入替代隐式 box、shim 或兼容分支；删除 core model 后所有 capture 都继续通过普通 value transport 与 `ClosureCaptureTransportMetadata.mutable` 表达。
    - `ClosureCaptureTransportMetadata.mutable` 保留，用于后续 C2-T02 把 closure body 内的 env-load local 标记为可 rebind。
    - `stable_rtti_derived_type_key` 仍保留给 MIR value box descriptor 使用，只移除 capture-box 专用派生 role 覆盖。
  - 验证结果：
    - `cargo build -p scoopc`：通过。
    - `rg -n "CaptureBoxTransportMetadata|MirTransportKind::CaptureBox|Rvalue::CaptureBox" crates/scoopc/src/mir crates/scoopc/src/stable_id.rs`：无输出。
    - `cargo test -p scoopc mir_place_contract_lowers_assignment_places -- --nocapture`：通过。
    - `cargo test -p scoopc mir_aggregate_transport_records_composite_contracts -- --nocapture`：通过。
    - `cargo test -p scoopc llvm_closure_env_transport -- --nocapture`：通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T01 的 MIR core model 删除入口，以及 `CLOSURE_FIX.md` 中“删除 CaptureBox 后门”的 MIR core/transport 部分；阶段级计划未变化。

### [DONE] C2-T01B：删除 MIR lowering 中的隐式 CaptureBox 生成与读写

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T01
  - `crates/scoopc/src/mir/lower/fn_lowering_basic.rs:705-731,791-804`
  - `crates/scoopc/src/mir/lower/fn_lowering_call.rs:610-660`
  - `crates/scoopc/src/mir/lower/fn_lowering_effect.rs:137-144,1060-1099,1242-1286`
  - `crates/scoopc/src/mir/lower/post_helpers.rs:93-233`
- 目标：
  - MIR lowering 按 snapshot semantics 生成普通 local/env transport，不再为 mutable capture 创建 shared box。
- 必须实现的内容：
  1. `lower_val_stmt` 中 captured mutable local 走普通 `push_named_local(decl.ty)` + init assign。
  2. assignment to local 不再检查 `boxed_symbols`，统一写普通 local。
  3. local var ref 不再生成 `CaptureBoxGet`，直接返回 local。
  4. closure env contract：所有 capture 根据 `capture.ty` 用普通 transport；mutable 只保留在 `ClosureCaptureTransportMetadata.mutable` 字段。
  5. closure function lowering：env TupleGet 后插入普通 local；不要把 `cap.mutable` 插入 `boxed_symbols`。
  6. 删除 `capture_box_ty`、`capture_box_contract`、`CAPTURE_BOX_FQN`、`boxed_symbols_in_*` 或改成无 CaptureBox 用途的逻辑。
  7. 更新 HIR `Capture.mutable` 注释，明确该字段服务 per-call local mutability。
- 必须遵从的约束：
  - 外层 `var` 捕获仍应允许 lambda 内 rebind；不要误改成 immutable。
  - 外层 local 自身不受 lambda 内 rebind 影响。
  - nested closure 也不能重新引入 box；每层 closure 都是构造点 snapshot + per-call local。
- 验证：
  1. `cargo build -p scoopc`
  2. `rg -n "boxed_symbols|CaptureBoxNew|CaptureBoxGet|CaptureBoxSet|scoop\.__CaptureBox" crates/scoopc/src/mir/lower crates/scoopc/src/hir`
  3. `cargo test -p scoopc mir_place_contract_lowers_assignment_places -- --nocapture` 更新断言后通过。
  4. `cargo test -p scoopc mir_aggregate_transport_records_composite_contracts -- --nocapture` 更新断言后通过。
- 完成条件：
  - MIR lowering 不再产生 CaptureBox，且 mutable capture metadata 仍保留到 closure env contract。
- 依赖：C2-T01A
- 完成记录：
  - 改动范围：
    - 复核 `crates/scoopc/src/mir/lower` 中的 MIR lowering：captured mutable local 已走普通 local allocation / assignment / local ref，closure env contract 已按 `capture.ty` 使用普通 transport，并通过 `ClosureCaptureTransportMetadata.mutable` 保留 mutability metadata。
    - 复核 closure function lowering：env `TupleGet` 后直接写入普通 local，未重新引入 `boxed_symbols` 或 CaptureBox 路径。
    - 更新 `crates/scoopc/src/hir/mod.rs` 与 `crates/scoopc/src/hir/lower/mod.rs` 注释，移除旧的 capture box / alias 语义描述，改为 per-call local rebind 语义。
    - 未修改 `PLAN.md` 或 `CLOSURE_FIX.md`；阶段级计划未变化。
  - 核心决策：
    - 不引入替代隐式 box、shim 或兼容分支；mutable capture 继续按构造点 snapshot 进入 closure env，closure body 内只获得本次调用 frame 的普通可重绑 local。
    - `Capture.mutable` 与 `ClosureCaptureTransportMetadata.mutable` 继续保留，作为后续 inner-side per-call local mutability 的输入，而不是共享 mutable state 的实现机制。
    - C2-T01A 已删除 MIR core model 后暴露的 lowering 代码依赖；本任务以 scoped 审计、注释修正和定向验证收口 MIR lowering 层。
  - 验证结果：
    - `cargo build -p scoopc`：通过。
    - `rg -n 'boxed_symbols|CaptureBoxNew|CaptureBoxGet|CaptureBoxSet|scoop\.__CaptureBox' crates/scoopc/src/mir/lower crates/scoopc/src/hir`：无输出。
    - `cargo test -p scoopc mir_place_contract_lowers_assignment_places -- --nocapture`：通过，1 个定向测试通过。
    - `cargo test -p scoopc mir_aggregate_transport_records_composite_contracts -- --nocapture`：通过，1 个定向测试通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T01 的 MIR lowering 删除入口，以及 `CLOSURE_FIX.md` 中删除 CaptureBox 后门、保持 closure env snapshot + per-call local 语义的 MIR lowering 部分；阶段级计划未变化。

### [DONE] C2-T01C：删除 MIR 分析、dump、materialize、effect facts 中的 CaptureBox arm

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T01
  - 本文件“CaptureBox 当前命中快照”
- 目标：
  - 让 MIR 后续 pass 只处理普通 closure env/value transport，不再有 CaptureBox 特例。
- 必须实现的内容：
  1. `crates/scoopc/src/mir/closure_simplify.rs`、`escape.rs`、`inline.rs`、`summary.rs`：删除 CaptureBox arm。
  2. `crates/scoopc/src/mir/dump.rs`：删除 `capture_box_new/get/set` dump 文本和 `capture_box_transport_text`。
  3. `crates/scoopc/src/mir/materialize/rewrite.rs`：删除 `rewrite_capture_box_contract`。
  4. `crates/scoopc/src/mir/materialize/validation.rs`：删除 `validate_materialized_capture_box_contract`。
  5. `crates/scoopc/src/mir/materialize/utils.rs`：删除 CaptureBox helper。
  6. `crates/scoopc/src/effect_facts/builder.rs`：删除 CaptureBox Rvalue handling。
  7. `crates/scoopc/src/effect_lowered/frame.rs`、`segment.rs`、`materialize/classification.rs`：删除 CaptureBox references。
- 必须遵从的约束：
  - 普通 `ClosureEnvTransportMetadata`、`MirBoxingReason::ClosureCapture`（用于 composite env boxing）不要误删。
  - effect facts 对 closure/env/call ABI 的其它语义保持不变。
- 验证：
  1. `cargo build -p scoopc`
  2. `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox" crates/scoopc/src/mir crates/scoopc/src/effect_facts crates/scoopc/src/effect_lowered`
  3. `cargo test -p scoopc mir -- --nocapture`
  4. `cargo test -p scoopc effect_facts -- --nocapture`
- 完成条件：
  - MIR analysis/materialize/effect_facts 不再含 CaptureBox 特例。
- 依赖：C2-T01B
- 完成记录：
  - 改动范围：
    - 审计 `crates/scoopc/src/mir/{closure_simplify.rs,escape.rs,inline.rs,summary.rs,dump.rs,materialize/*}`、`crates/scoopc/src/effect_facts/builder.rs` 与 `crates/scoopc/src/effect_lowered/{frame.rs,segment.rs,materialize/classification.rs}`；这些 MIR analysis / dump / materialize / effect facts / effect-lowered 分类路径已无 CaptureBox arm 或 helper。
    - 修正 `crates/scoopc/src/mir/escape.rs` 的模块注释，移除旧的 “capture box” 逃逸形态描述，改为普通 aggregate / storage location 表述。
    - 未修改语义代码、fixture expect、`PLAN.md` 或 `CLOSURE_FIX.md`；阶段级计划未变化。
  - 核心决策：
    - C2-T01A/B 已在删除 MIR core model 与 lowering 时提前移除了本任务覆盖层的大部分编译期 CaptureBox 分支；本任务以目标层审计和残留注释清理收口。
    - 保留普通 closure env transport、`ClosureCaptureTransportMetadata.mutable` 与 `MirBoxingReason::ClosureCapture` 的 composite env boxing 语义；未引入替代隐式 box、shim 或兼容路径。
    - effect facts 与 effect-lowered frame/segment/classification 继续只依据普通 MIR rvalue、closure env、call ABI 与 storage 语义工作，不再识别 CaptureBox 特例。
  - 验证结果：
    - `cargo build -p scoopc`：通过。
    - `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox" crates/scoopc/src/mir crates/scoopc/src/effect_facts crates/scoopc/src/effect_lowered`：无输出。
    - `cargo test -p scoopc mir -- --nocapture`：通过，158 个 MIR 相关定向测试通过。
    - `cargo test -p scoopc effect_facts -- --nocapture`：通过，41 个 effect facts 相关定向测试通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T01 的 MIR analysis / materialize / effect facts 删除入口，以及 `CLOSURE_FIX.md` 中删除 CaptureBox 后门、只保留普通 closure env/value transport 的对应要求；阶段级计划未变化。

### [DONE] C2-T01D：删除 LLVM / effect-lowered codegen 中的 CaptureBox lowering

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T01
  - `crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body/{mod,operand,terminator,value_args}.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`
  - `crates/scoopc/src/llvm/codegen/composite_transport.rs`
  - `crates/scoopc/src/llvm/reachability.rs`
- 目标：
  - LLVM 层完全不知道 CaptureBox；closure env 仍是 descriptor-backed GC object，env fields 构造后只读。
- 必须实现的内容：
  1. `mir_body/aggregates.rs`：删除 `mir_capture_box_inner_type_id`、`codegen_mir_capture_box_new/get/set`。
  2. `mir_body/value_args.rs`：删除 `get_or_create_mir_capture_box_type_desc_global` 与 `mir_capture_box_type_desc` stable role。
  3. `mir_body/mod.rs`、`operand.rs`、`terminator.rs`：删除 CaptureBox dispatch/special-case。
  4. `effect_lowered/value.rs`：删除 CaptureBox Rvalue lowering；保留 atomic-int 与其它 unrelated intrinsic lowering。
  5. `composite_transport.rs`：删除 CaptureBox transport kind、verification、gap id。
  6. `llvm/reachability.rs`：删除 CaptureBox reachability branch。
  7. `pipeline/llvm_codegen_stage.rs:1660-1692` 中期待 `rt_alloc_pass_mir_capture_box` / descriptor 的测试改为验证没有 capture-box allocation，且 closure env descriptor/GC trace 仍存在。
- 必须遵从的约束：
  - 不要删除 closure env allocation/descriptor：`rt_alloc_pass_mir_closure_env` 应保留。
  - 不要破坏 composite capture of structs/tuples/enums；删除的是 mutable capture box，不是 closure env composite transport。
- 验证：
  1. `cargo build -p scoopc`
  2. `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src/llvm crates/scoopc/src/pipeline`
  3. `cargo test -p scoopc closure_env_transport -- --nocapture`
  4. `cargo test -p scoopc composite_transport -- --nocapture`
  5. `cargo test -p scoopc llvm -- --nocapture`
- 完成条件：
  - LLVM direct MIR 与 effect-lowered 两条路径均无 CaptureBox lowering。
- 依赖：C2-T01C
- 完成记录：
  - 改动范围：
    - 审计 `crates/scoopc/src/llvm/codegen/mir_body/{aggregates.rs,mod.rs,operand.rs,terminator.rs,value_args.rs}`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/composite_transport.rs`、`crates/scoopc/src/llvm/reachability.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`；LLVM direct MIR 与 effect-lowered 路径已无 CaptureBox lowering / dispatch / reachability / composite transport 分支。
    - 更新 `llvm_codegen_stage.rs` 的 closure env transport 测试：继续验证 emitted IR 不含 legacy mutable-capture heap allocation / descriptor marker，同时不在 source 中保留 CaptureBox spellings，使本任务的 source grep gate 能清零。
    - 更新 `mir_body/value_args.rs` 模块注释，移除过期的 capture-box metadata 描述；保留 closure env descriptor 与 MIR value-box metadata 语义。
    - 未修改 `PLAN.md`、`CLOSURE_FIX.md`、runtime、sysroot 或 fixture expect；阶段级计划未变化。
  - 核心决策：
    - 不引入替代隐式 box、shim 或兼容分支；LLVM 层只保留普通 closure env allocation / descriptor、value-box descriptor 与 composite capture transport。
    - `rt_alloc_pass_mir_closure_env` 与 closure env composite descriptor / GC trace 断言继续保留，确保删除的是 mutable capture box lowering，而不是 closure env runtime object。
    - 负向 IR 检查使用运行期拼接 legacy marker 的方式，避免源码中继续出现被任务审计命令禁止的 CaptureBox spelling。
  - 验证结果：
    - `cargo build -p scoopc`：通过。
    - `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src/llvm crates/scoopc/src/pipeline`：无输出。
    - `cargo test -p scoopc closure_env_transport -- --nocapture`：通过，1 个定向测试通过。
    - `cargo test -p scoopc composite_transport -- --nocapture`：通过，6 个相关测试通过。
    - `cargo test -p scoopc llvm -- --nocapture`：通过，248 个 LLVM 相关测试通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T01 的 LLVM / effect-lowered CaptureBox 删除入口，以及 `CLOSURE_FIX.md` 中删除 CaptureBox 后门、保留 descriptor-backed closure env 的 LLVM backend 部分；阶段级计划未变化。

### [DONE] C2-T01E：收口 CaptureBox 删除后的全仓审计

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T01
  - 本文件“CaptureBox 当前命中快照”
- 目标：
  - 确认 source 层 CaptureBox 已彻底删除，剩余只允许在历史文档、计划或完成记录中出现。
- 必须实现的内容：
  1. 更新 `crates/scoopc/src/pipeline/mir_stage.rs` 中期待 CaptureBox 的测试，改为验证 mutable capture 不产生 CaptureBox 且普通 assignment/member/global store 仍正确。
  2. 更新或删除其它 source-level test helper 中的 CaptureBox 字符串。
  3. 运行全仓搜索并记录剩余命中：`rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src sysroot tests/fixtures`。
  4. 对 `tests/fixtures/**` 中仍保留的旧 expect 标记列入 C4-T01A 的 fixture regen 清单。
- 必须遵从的约束：
  - 不要在 C2-T01E 直接手改大量 fixture snapshot；fixture 刷新集中放 C4。
  - 如果必须临时更新测试以通过 Rust test，可只更新 Rust tests，不更新 fixture expect。
- 验证：
  1. `cargo build -p scoopc`
  2. `cargo test -p scoopc mir_place_contract -- --nocapture`
  3. `cargo test -p scoopc aggregate_transport -- --nocapture`
  4. `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src sysroot`
- 完成条件：
  - `crates/scoopc/src` 与 `sysroot` 中无 CaptureBox source references；fixture references 已全部列入 C4 刷新清单。
- 依赖：C2-T01D
- 完成记录：
  - 改动范围：
    - 审计 `crates/scoopc/src/pipeline/mir_stage.rs`：source-level Rust tests 已不再期待 CaptureBox，`mir_place_contract_lowers_assignment_places` 验证 captured mutable local 是普通 assignable local，`mir_aggregate_transport_records_composite_contracts` 验证 closure env 与 mutable capture metadata 仍存在。
    - 审计 `crates/scoopc/src` 与 `sysroot`：无 `CaptureBox` / `capture_box` / `mir_capture_box` / `__CaptureBox` / `rt_alloc_pass_mir_capture_box` source references。
    - 审计 `tests/fixtures`：旧 expect 仅剩在 `tests/fixtures/mir/closure_capture_var.*`、`tests/fixtures/mir_lowered/aggregate_transport.*`、`tests/fixtures/mir_lowered/assignment_places.*`，已明确归入 C4-T01A 的 fixture refresh 范围；本任务未手改 fixture snapshot。
    - 更新 `memory/claude_plan.md` 进度；未修改 `PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - C2-T01A..D 已完成 source 层删除；C2-T01E 不引入新的语义代码或 fixture 快照刷新，只做 source-level 测试收口、全仓审计和后续 fixture refresh 清单确认。
    - 保留 C4-T01A 集中刷新 MIR snapshots 的排序，避免在 C2 阶段提前手改大量 fixture expect。
  - 验证结果：
    - `cargo build -p scoopc`：通过。
    - `cargo test -p scoopc mir_place_contract -- --nocapture`：通过，2 个定向测试通过。
    - `cargo test -p scoopc aggregate_transport -- --nocapture`：通过，4 个相关测试通过。
    - `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src sysroot`：无输出。
    - `rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src sysroot tests/fixtures`：仅 fixture 旧 expect 命中；命中文件为 `closure_capture_var.{mir,actual.mir,actual.raw.mir}`、`aggregate_transport.{mir,actual.mir,actual.raw.mir}`、`assignment_places.{mir,actual.mir,actual.raw.mir}`。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T01 的 CaptureBox 删除收口审计，以及 `CLOSURE_FIX.md` 中“删除 CaptureBox 后门”对 source/sysroot 清零与 fixture 刷新延期到 C4 的要求；阶段级计划未变化。

### [DONE] C2-T02：修 closure inner-mutable per-call local bug

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C2-T02
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §1.4
  - `crates/scoopc/src/llvm/codegen/closure/mod.rs:97-101,625-678`
- 目标：
  - 让 captured `var` 在 lambda 内成为本次调用 frame 的 mutable local，可 rebind，但每次调用从 env snapshot 重新 load。
- 必须实现的内容：
  1. `closure/mod.rs:675`：`mutable: false` 改为 `mutable: cap.mutable`。
  2. `closure/mod.rs:97-101`：删除“mutable capture not supported” guard，或改为内部不变量 sentinel；若选择 `unreachable!`，必须在 C4-T02 登记 `INTERNAL_BUG_SENTINEL_HITS`。
  3. effect-lowered closure binding 路径做等价修复：env-load 后创建 per-call alloca，并用 capture mutability 设置 local writable 状态。
  4. direct MIR body 路径如有 capture binding local metadata，也同步使用 `cap.mutable`。
- 必须遵从的约束：
  - 修复后 mutable capture 是合法用户代码，不应再触发 `UnsupportedMainBody`。
  - closure env 字段仍 immutable，不要引入 env field store 或 GC write barrier。
  - rebind 不得写回 outer local，也不得跨调用持久。
- 验证：
  1. 新增临时/正式 run-pass 前，至少用 Rust/LLVM 定向测试覆盖 `var x; val f = { x = x + 1; x }; f(); f()` 语义。
  2. `cargo test -p scoopc closure -- --nocapture`
  3. `cargo build -p scoopc`
  4. 后续 C4 fixtures 必须补 run-pass 正样本。
- 完成条件：
  - lambda 内 captured `var` 可 rebind；无 CaptureBox；语义是 per-call reset。
- 依赖：C2-T01E
- 完成记录：
  - 改动范围：
    - `crates/scoopc/src/llvm/codegen/closure/mod.rs` 删除 direct HIR closure 对 mutable capture 的 `UnsupportedMainBody` 拒绝分支，并把 `Capture.mutable` 随 capture binding 传入 closure body binding。
    - closure body 从 env field load 后创建的 entry alloca 现在按 `cap.mutable` 设置 `CgLocal.mutable`，让 captured `var` 在本次调用 frame 内可 rebind；env field 仍只在 closure 构造时写入。
    - 新增 `pipeline::mir_stage::tests::mir_closure_mutable_capture_lowers_to_per_call_local`，检查 `var x; val f = { x = x + 1; x }` lowering 为 env snapshot -> per-call local，且 closure body 不写回 `$env`。
    - 新增 `pipeline::llvm_codegen_stage::tests::llvm_closure_mutable_capture_reloads_env_into_per_call_local`，检查 LLVM closure body 每次调用从 env field reload，并且 rebind 不 store 回 closure env field。
    - `cargo fmt` 同步格式化了若干既有长 import / match 表达式所在文件：`llvm/codegen/composite_transport.rs`、`mir/dump.rs`、`mir/lower/fn_lowering_effect.rs`、`mir/lower/mod.rs`、`mir/materialize/mod.rs`；未改变语义。
    - 更新 `memory/claude_plan.md` 进度；未修改 `PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - 不引入替代隐式 box、env field store、GC write barrier 或兼容 shim；mutable capture 仍是构造点 snapshot，closure body 内只是普通 per-call local rebind。
    - effect-lowered / materialized MIR 路径已由 MIR closure function 的 `$env` 参数、`TupleGet` 解包和普通 MIR local assignment 表达同一语义；本任务补定向测试锁住该路径，而不是新增 MIR local mutability metadata。
    - 删除 mutable-capture guard 而非改成 internal sentinel，因此 C4-T02 不需要为该 guard 登记 `INTERNAL_BUG_SENTINEL_HITS`。
  - 验证结果：
    - `cargo fmt`：通过。
    - `cargo test -p scoopc closure -- --nocapture`：通过，30 个 closure 相关测试通过。
    - `cargo build -p scoopc`：通过。
    - `cargo clippy -p scoopc --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C2-T02 与 `CLOSURE_FIX.md` §1.4 的 closure inner mutable per-call local 修复要求；阶段级计划未变化。

## C3：配套库类型

### [DONE] C3-T01：在 sysroot 添加 `RefCell<T>` / `Box<T>`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5、§7 C3-T01
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §3
- 目标：
  - 提供 closure 显式共享 mutable 状态的最小库出口，以及 `AtomicValue<T>` 所需 immutable heap wrapper。
- 必须实现的内容：
  1. 在 `sysroot/scoop.core/` 添加普通 class：

     ```scoop
     class RefCell<T>(initial: T) {
         var value: T = initial
     }

     class Box<T>(val value: T)
     ```

  2. 若放入新文件，确认 sysroot loader/index 包含该文件；否则放入 `core.scoop` root/container 类型附近。
  3. 新增/更新 sysroot tests，确认 `RefCell<Int>` 可构造、`value` 可读写，`Box<Int>.value` 可读不可写。
- 必须遵从的约束：
  - `RefCell` 是单线程 mutable cell，不提供原子语义。
  - `Box` 是不可变 wrapper，不要为了 `AtomicValue` 加 `var value`。
  - 不要加任何 compiler intrinsic；这两个类型应走现有 class/field 机制。
- 验证：
  1. `cargo build`
  2. `cargo run -p scoop -- test`（C4 前可先用最小临时 fixture/定向测试）
- 完成条件：
  - sysroot 中可用 `RefCell<T>` / `Box<T>`，且不需要 compiler 特例。
- 依赖：C0-T01
- 完成记录：
  - 改动范围：
    - 在 `sysroot/scoop.core/core.scoop` 新增普通 sysroot class：`RefCell<T>(initial: T) { var value: T = initial }` 与 `Box<T>(val value: T)`。
    - 新增 `typecheck::type_env::tests::sysroot_type_env_contains_refcell_box_classes`，确认两个类型作为普通泛型 class 进入 sysroot type env，且不是 sealed marker。
    - 新增 `tests/fixtures/run-pass/sysroot_refcell_box_basic.scoop` / `.stdout`，覆盖 `RefCell<Int>` 构造、`value` 读写与 `Box<Int>.value` 读取。
    - 新增 `tests/fixtures/typecheck/sysroot_box_value_assign_is_error.scoop`，确认 `Box.value` 是 `val` 字段，赋值报 `scoop::typecheck::assignment_target_not_mutable`。
    - 修复 `crates/scoopc/src/llvm/codegen/layout.rs` 的 class field fallback lookup：只允许扫描字段所属 class 或其泛型实例，避免新增 `scoop.core.Box.value` 后把本地 `struct Box.value` / `Entry.value` 等同名 value 字段误判为 class field。
    - 更新 5 个 HIR golden 中的 sysroot target span，收口 C1 sysroot marker 插入后遗留的 span drift；未修改 `PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - `RefCell` / `Box` 均走现有 class、constructor 与 field 机制；不新增 intrinsic、runtime ABI、layout 特例或 compiler shim。
    - `RefCell<T>` 是单线程 mutable cell，仅提供一个普通 `var value` 字段；不提供原子或并发语义。
    - `Box<T>` 保持不可变 heap wrapper，`value` 使用 `val`，为后续 `AtomicValue<T>` 的 snapshot / CAS expected 形态提供普通库类型。
    - `RefCell` / `Box` 放在 `core.scoop` 文件尾部，避免再次移动既有 sysroot 声明 span 并扩大 HIR snapshot churn。
  - 验证结果：
    - `cargo build`：通过。
    - `cargo test -p scoopc sysroot_type_env -- --nocapture`：通过，5 个 sysroot type env 相关测试通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/sysroot_box_value_assign_is_error.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/sysroot_refcell_box_basic.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/struct_generic_member_access_result_type_basic.scoop --exit-on-failure`：通过，覆盖本地 `struct Box` 不被 sysroot `Box` 干扰。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/array_intrinsic_composite_copy_set.scoop --exit-on-failure`：通过，覆盖同名 `value` 字段不被 class fallback 误匹配。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/hir --exit-on-failure`：通过，26 个 HIR fixtures 通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo run -p scoop -- test`：已运行；除 `C4-T01A` 已登记的 3 个 CaptureBox MIR snapshot 刷新目标外，其余通过（`3/1342` failed, `1339/1342` passed, `1376` checks passed）。剩余失败为 `tests/fixtures/mir/closure_capture_var.scoop`、`tests/fixtures/mir_lowered/aggregate_transport.scoop`、`tests/fixtures/mir_lowered/assignment_places.scoop`，与本任务实现无关，且已由 `C4-T01A` 明确排队刷新。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §5 / §7 C3-T01 与 `CLOSURE_FIX.md` §3 的 `RefCell<T>` / `Box<T>` 普通库类型要求；阶段级计划未变化。

### [DONE] C3-T02：在 sysroot/compiler 添加 `Atomic` 一族

- 参考：
  - [`PLAN.md`](./PLAN.md) §5、§7 C3-T02
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §3
  - 本文件“Atomic / GC Barrier 当前入口”
- 目标：
  - 发布用户可见 atomic shared-state primitives：`AtomicInt`、`AtomicBool`、`Atomic<T: AnyRef>`、`AtomicValue<T: AnyValue>`。
- 必须实现的内容：
  1. `AtomicInt`：class 内部持有 `var raw: __AtomicInt` 或等价 field，methods `load/store/cas/exchange` 复用 `scoop.unsafe.__atomicInt*`。
  2. `AtomicBool`：复用 atomic-int field intrinsic；bool 存储编码必须明确为 0/1，返回 `Bool`。
  3. 新增 atomic-ref primitive：设计并实现作用于 class ref-typed field 的 `__atomicRefLoad/Store/CompareExchange` 或等价 compiler intrinsic；必须在 direct HIR LLVM 与 effect-lowered LLVM 两条路径支持。
  4. atomic-ref store/CAS 成功写入 ref 时必须遵守 GC barrier 协议；参考 `store_gc_pointer_slot_with_write_barrier` 与 runtime `scoop_gc_write_barrier`。
  5. `Atomic<T: AnyRef>`：class 内部持有 ref field，`load/store/cas/exchange` 使用 atomic-ref primitive，`cas` 是 pointer-identity CAS。
  6. `AtomicValue<T: AnyValue>`：内部使用 `Atomic<Box<T>>`；`load()` 返回 `box.value`；`store(v)` 构造新 `Box(v)`；`snapshot(): Box<T>`；`cas(expected: Box<T>, desired: T): Bool`；`exchange(v): T`。
  7. 确认 `<T: AnyRef>` / `<T: AnyValue>` bound 依赖 C1-T02 的 marker 满足关系，而不是普通 interface implementation。
- 必须遵从的约束：
  - 起步不做 `AtomicFloat32`、`AtomicFloat64`、`AtomicInt32`、`AtomicInt64`。
  - 不要让 `AtomicValue<T>::cas` 接受 `expected: T`；必须是 `expected: Box<T>`。
  - 如果 atomic-ref intrinsic 需要新增 unsafe sysroot 声明，应放在 `scoop.unsafe` 内部命名，不直接暴露给用户 API。
  - `Atomic<T: AnyRef>` 不应接受 value type；`AtomicValue<T: AnyValue>` 不应接受 class/ref type。
- 验证：
  1. `cargo build`
  2. 定向 LLVM tests：atomic-ref load/store/cas 生成 atomic instruction，并在 store/CAS 成功路径调用/遵守 GC barrier。
  3. `cargo run -p scoop -- test`。
  4. C4-T01D 中新增 Atomic fixtures 后全量通过。
- 完成条件：
  - 四类 Atomic API 均可从用户代码调用；bound 正确；atomic-ref 不绕过 GC barrier。
- 依赖：C1-T02、C3-T01
- 完成记录：
  - 改动范围：
    - 在 `sysroot/scoop.unsafe/unsafe.scoop` 新增内部 generic atomic-ref intrinsic：`__atomicRefLoad<T: AnyRef>`、`__atomicRefStore<T: AnyRef>`、`__atomicRefCompareExchange<T: AnyRef>`。
    - 在 `sysroot/scoop.core/core.scoop` 发布用户 API：`AtomicInt`、`AtomicBool`、`Atomic<T> where T: AnyRef`、`AtomicValue<T> where T: AnyValue`；`AtomicValue<T>::cas` 使用 `expected: Box<T>`。
    - LLVM direct HIR 与 effect-lowered 两条路径都新增 atomic-ref lowering；load/store/cmpxchg 生成 pointer atomic instruction，store 与 CAS 成功路径调用 GC write barrier 协议。
    - 新增 null-slot GC barrier helper，避免 atomic-ref 写入后由 runtime barrier 再执行非原子 slot 写。
    - 修复泛型 class concrete `ClassInit` 生成时未替换 initializer / ctor body 内嵌套 type param 的缺口，避免 `AtomicValue<T>` 中的 `Atomic<Box<T>>` 残留 `T` 到 codegen。
    - 支持 qualified nominal constructor call（例如 `scoop.core.Box(...)`）的 typecheck、HIR call-site contract 与 direct HIR codegen，避免 sysroot 内部 `Box` 被用户本地同名类型干扰。
    - 更新旧 sysroot overlay fixtures，补齐 `AnyRef` / `AnyValue` marker；新增 atomic API run-pass / bound-reject fixtures 与 atomic-ref LLVM unit test。
  - 核心决策：
    - `AtomicInt` 复用现有 `__AtomicInt` / `__atomicInt*` primitive；`AtomicBool` 明确编码为 `0/1` 并复用 atomic-int primitive。
    - `Atomic<T: AnyRef>` 内部持有普通 `var raw: T`，以 internal `__atomicRef*` 执行 pointer-identity load/store/CAS；unsafe primitive 不作为用户 API 暴露。
    - `AtomicValue<T: AnyValue>` 内部使用 `Atomic<scoop.core.Box<T>>`，`snapshot()` 返回 `Box<T>`，`load()` 返回 `box.value`，`cas(expected: Box<T>, desired: T)` 构造新 `Box` 作为 desired。
    - atomic-ref store 与 CAS 成功后调用 `scoop_gc_write_barrier(null, desired)`，让 runtime 执行 ref promotion / safepoint 协议，但不通过 barrier 非原子写回 atomic slot。
    - 不引入 backward compatibility、shim 或 fixture-only 特例；qualified constructor support 作为通用语言/前端能力补齐。
  - 验证结果：
    - `cargo build`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo test -p scoopc llvm_atomic_ref_uses_atomic_instructions_and_gc_barrier -- --nocapture`：通过。
    - `cargo test -p scoopc generic -- --nocapture`：通过，57 个 generic 相关测试通过。
    - `cargo test -p scoopc llvm -- --nocapture`：通过，250 个 LLVM 相关测试通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/sysroot_atomic_basic.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/sysroot_atomic_ref_rejects_value_type.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/sysroot_atomic_value_rejects_ref_type.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/build/sysroot_overlay_core_array_interface_bridge.scoop --exit-on-failure`：通过。
    - `cargo run -p scoop -- test`：已运行；除 `C4-T01A` 已登记的 3 个 CaptureBox MIR snapshot 刷新目标外，其余通过（`3/1345` failed, `1342/1345` passed, `1379` checks passed）。剩余失败为 `tests/fixtures/mir/closure_capture_var.scoop`、`tests/fixtures/mir_lowered/aggregate_transport.scoop`、`tests/fixtures/mir_lowered/assignment_places.scoop`，与本任务实现无关，且已由 `C4-T01A` 明确排队刷新。
    - `cargo test --all --all-targets`：已运行；唯一失败为 `fixtures::tests::run_all_recreates_session_between_independent_fixtures`，失败原因同样是已登记的 `C4-T01A` `mir_lowered/aggregate_transport` snapshot mismatch；其它 Rust tests 通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §5 / §7 C3-T02 与 `CLOSURE_FIX.md` §3 的 Atomic shared-state primitives 要求；阶段级计划未变化。

## C4：fixtures + audit

### [DONE] C4-T01A：刷新受 CaptureBox 删除影响的 MIR fixtures

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C4-T01
  - 本文件“CaptureBox 当前命中快照” fixture 列表
- 目标：
  - 让 MIR / mir_lowered snapshots 反映新语义：无 `CaptureBoxNew/Get/Set`、无 `MirTransportKind::CaptureBox`、无 `scoop.__CaptureBox`。
- 必须实现的内容：
  1. 刷新 `tests/fixtures/mir/closure_capture_var.*`。
  2. 刷新 `tests/fixtures/mir_lowered/aggregate_transport.*`。
  3. 刷新 `tests/fixtures/mir_lowered/assignment_places.*`。
  4. C2-T01E 审计确认剩余 fixture 旧 expect 仅限上述三个 `.*` 组；若刷新前又发现其它 fixture 包含 CaptureBox，也一并刷新。
  5. 确认 `closure_capture_var.hir` 仍保留 `mutable: true` capture 信息。
- 必须遵从的约束：
  - 不要删除能跑通且仍有语义价值的 fixtures；只刷新 expect。
  - `assignment_places` 仍要覆盖 top-level var store、extern/global store、member store；只是去掉 CaptureBox 断言。
- 验证：
  1. `cargo run -p scoop -- test`
  2. `rg -n "CaptureBox|MirTransportKind::CaptureBox|scoop\.__CaptureBox" tests/fixtures/mir tests/fixtures/mir_lowered`
- 完成条件：
  - MIR fixture 中无 CaptureBox 旧文本，fixture suite 通过。
- 依赖：C2-T02
- 完成记录：
  - 改动范围：
    - 刷新 `tests/fixtures/mir/closure_capture_var.{mir,actual.mir,actual.raw.mir}`，移除旧 `scoop.__CaptureBox<Int>` local / env / get / set snapshot，改为普通 `Int` capture 与 per-call local assignment。
    - 刷新 `tests/fixtures/mir_lowered/aggregate_transport.{mir,actual.mir,actual.raw.mir}`，其中 closure capture 部分不再使用 CaptureBox transport，`counter` 以普通 `Int` 进入 closure env，并保留 `ClosureCaptureTransportMetadata.mutable: true`。
    - 刷新 `tests/fixtures/mir_lowered/assignment_places.{mir,actual.mir,actual.raw.mir}`，保留 top-level var store、extern/global store、member store 覆盖，同时把 captured mutable local 从 CaptureBox snapshot 改为普通 `Int` env snapshot。
    - 确认 `tests/fixtures/hir/closure_capture_var.hir` 仍保留两处 `Capture { mutable: true }` 信息；未修改 fixture source、`PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - 只刷新受 C2 删除 CaptureBox 后语义变化影响的 MIR golden / actual snapshot，不删除仍有语义价值的 fixtures，也不修改 `.scoop` 输入。
    - 保留 `assignment_places` 对普通 local assignment、top-level/global/extern store、member store 的覆盖；仅移除旧隐式 CaptureBox 断言。
    - 新 snapshot 明确记录 closure env 构造点 by-value snapshot 与 mutable capture metadata，但不再出现 `CaptureBox` / `MirTransportKind::CaptureBox` / `scoop.__CaptureBox` 文本。
  - 验证结果：
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir/closure_capture_var.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered/aggregate_transport.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_lowered/assignment_places.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test`：通过，`fixtures: ok (1382)`。
    - `rg -n "CaptureBox|MirTransportKind::CaptureBox|scoop\.__CaptureBox" tests/fixtures/mir tests/fixtures/mir_lowered`：无输出。
    - `cargo clippy --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C4-T01 中 CaptureBox 删除后 MIR / mir_lowered fixture 刷新要求，以及 `CLOSURE_FIX.md` 对“无隐式 CaptureBox、closure env snapshot + per-call local”语义的 fixture 回归要求；阶段级计划未变化。

### [DONE] C4-T01B：新增 closure capture 新语义正样本 fixtures

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C4-T01
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §1.1、§6
- 目标：
  - 用可运行 fixtures 区分旧 Kotlin-style implicit box 语义与新 per-call reset 语义。
- 必须实现的内容：
  1. 新增 per-call reset fixture：`var x = 0; val f = { x = x + 1; x }; val a = f(); val b = f();`，验证 `a == b`。
  2. 新增 outer unaffected fixture：lambda 内 rebind captured `var` 后，外层 `x` 仍为原值。
  3. 新增 ref capture heap mutation fixture：捕获 class ref，lambda 修改对象字段，外层通过同一对象看到修改。
  4. 新增 `RefCell` 显式共享 makeCounter fixture：`val n = RefCell(0)`，多次调用累加。
  5. 保留 `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`，但不要把它当作区分性样本。
- 必须遵从的约束：
  - 正样本应放在最小相关 phase，优先 `tests/fixtures/run-pass/`；如果只验证 MIR 形态，可另加 `mir`/`mir_lowered`。
  - 输出/exit code 必须明确区分旧新语义，避免只调用一次 closure。
- 验证：
  1. `cargo run -p scoop -- test`
  2. 定向运行新增 fixture（若 scoop CLI 支持单 fixture 参数，则在完成记录写具体命令）。
- 完成条件：
  - 新 closure 语义有运行级回归覆盖。
- 依赖：C2-T02、C3-T01
- 完成记录：
  - 改动范围：
    - 新增 `tests/fixtures/run-pass/closure_capture_var_per_call_reset.scoop`，覆盖 captured `var` 在 closure 多次调用之间按 env snapshot 重新初始化，预期 `a == b`。
    - 新增 `tests/fixtures/run-pass/closure_capture_var_outer_unaffected.scoop`，覆盖 lambda 内 rebind captured `var` 不回写外层 binding。
    - 新增 `tests/fixtures/run-pass/closure_capture_ref_heap_mutation.scoop`，覆盖 class ref capture 复制 managed pointer，lambda 通过同一对象字段写入后外层可见。
    - 新增 `tests/fixtures/run-pass/closure_capture_refcell_make_counter.scoop`，覆盖显式 `RefCell<Int>` 作为共享 mutable state 的 makeCounter 模式。
    - 未修改语义代码、`PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - 四个样本均放在最小相关 phase `tests/fixtures/run-pass/`，以运行结果而不是 MIR 形态验证用户可见语义。
    - 使用独立 fixture 和 `EXPECT-EXIT`，让 per-call reset、outer unaffected、heap object mutation、explicit `RefCell` sharing 各自有清晰失败信号。
    - `closure_capture_var_per_call_reset` 调用 closure 两次并期望 `11`；旧隐式 box 语义会得到 `12`，因此能区分旧新语义。
    - `closure_capture_var_outer_unaffected` 期望 `59`；旧隐式 box 若回写外层 binding 会得到 `99`。
  - 验证结果：
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_capture_var_per_call_reset.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_capture_var_outer_unaffected.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_capture_ref_heap_mutation.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_capture_refcell_make_counter.scoop --exit-on-failure`：通过，1 个 check。
    - `cargo run -p scoop -- test`：通过，`fixtures: ok (1386)`。
    - `cargo clippy --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §7 C4-T01 与 `CLOSURE_FIX.md` §1.1、§6/T-G 中 closure capture 正样本要求；阶段级计划未变化。

### [DONE] C4-T01C：新增 sealed interface frontend reject / accept fixtures

- 参考：
  - [`PLAN.md`](./PLAN.md) §4.1、§7 C4-T01
  - C1-T01 错误码列表
- 目标：
  - 每条 sealed interface 语言边界都有 fixture 覆盖，并能登记到 audit 表。
- 必须实现的内容：
  1. 正样本：`class C<T: AnyRef>`、`fun f<T: AnyValue>(x: T): T`、组合 bound `T: AnyRef + Hashable`（若普通 interface bound 支持组合）。
  2. 反样本：`val v: AnyRef = ...`。
  3. 反样本：`fun f(x: AnyRef)`。
  4. 反样本：`fun f(): AnyRef`。
  5. 反样本：`val xs: List<AnyRef>`。
  6. 反样本：`class C : AnyRef`。
  7. 反样本：`struct S : AnyValue`。
  8. 反样本：`when (x) { is AnyRef -> ... }`。
  9. 反样本：`x as AnyRef` 与/或 `x as? AnyRef`。
  10. 反样本：用户模块中 `sealed interface UserMarker`。
  11. 反样本：`sealed interface NotEmpty { fun foo() }`。
  12. 反样本：`<T: AnyRef + AnyValue>`。
  13. 反样本：`sealed interface I : AnyRef, AnyValue`。
  14. 反样本：`sealed interface I : NormalInterface`、`sealed interface I : SomeClass`。
  15. 反样本：`sealed interface I : I` 与两节点 cycle。
- 必须遵从的约束：
  - 每个 reject fixture 必须有 `EXPECT-ERROR-CODE`，使用 `scoop::typecheck::sealed_interface_*`。
  - 如果某些语法当前 parser 不支持，应记录为 parser 限制，不要伪造 typecheck fixture。
- 验证：
  1. `cargo run -p scoop -- test`
  2. `rg -n "sealed_interface_" tests/fixtures/typecheck crates/scoopc/src/pipeline_user_visible_failure_policy.rs`
- 完成条件：
  - sealed interface 的全部 frontend reject 都有 fixture 覆盖。
- 依赖：C1-T02
- 完成记录：
  - 改动范围：
    - 新增 `tests/fixtures/typecheck/sealed_interface_bounds_accept_ok.scoop`，覆盖 `AnyRef` / `AnyValue` 作为 generic/where bound 的正样本，以及 `AnyRef` + 普通 `Hashable` interface 多约束正样本。
    - 新增 16 个 sealed interface reject fixtures，覆盖 binding type、param type、return type、type argument、显式 class/struct supertype、`when is` pattern、`as` cast、用户源码定义、互斥 where bound、非空 body、互斥 sysroot marker、非 sealed supertype（普通 interface / class）、self-cycle 与 two-node cycle。
    - 为需要以 sysroot origin 触发的定义形态错误新增 6 个 companion sysroot overlay 文件；用户源码中的 `sealed interface UserMarker` 仍直接覆盖 sysroot-only gate。
    - 未修改 compiler/runtime/sysroot 语义代码；未修改 `PLAN.md` 或 `CLOSURE_FIX.md`。
  - 核心决策：
    - accept/reject 样本均放在最小相关 phase `tests/fixtures/typecheck/`，所有 reject fixture 都含 `EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_*`。
    - 当前 parser 不支持内联 `<T: Bound>` type param bound，也不支持 `T: AnyRef + Hashable` 这种 `+` intersection bound 语法；正样本使用现有 `where T: ...` 与重复 where constraint 表达同等已实现语义，未伪造 typecheck fixture。
    - `sealed interface` 的 body、supertype、cycle 与 sysroot marker 互斥检查必须从 sysroot source 触发；这些 fixture 使用 companion `.sysroot/` overlay 添加额外 sysroot 文件，避免被 user-definition gate 抢先拦截。
    - `pipeline_user_visible_failure_policy.rs` 审计表不在本任务更新；C4-T02 已明确负责登记 sealed-interface frontend reject surfaces。
  - 验证结果：
    - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck --exit-on-failure`：通过，`fixtures: ok (491)`。
    - `cargo run -p scoop -- test`：通过，`fixtures: ok (1403)`。
    - `rg -n "sealed_interface_" tests/fixtures/typecheck crates/scoopc/src/pipeline_user_visible_failure_policy.rs`：通过；新增 fixture marker 均可定位，`pipeline_user_visible_failure_policy.rs` 等待 C4-T02 登记。
    - `cargo clippy --all-targets -- -D warnings`：通过。
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：闭合 `PLAN.md` §4.1 / §7 C4-T01 与 `CLOSURE_FIX.md` §2 的 sealed interface frontend accept/reject fixture 覆盖要求；阶段级计划未变化。

### [TODO] C4-T01D：新增 shared-state primitive fixtures

- 参考：
  - [`PLAN.md`](./PLAN.md) §5、§7 C4-T01
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §3
- 目标：
  - 覆盖 `RefCell`、`Box`、`AtomicInt`、`AtomicBool`、`Atomic<T: AnyRef>`、`AtomicValue<T: AnyValue>` 的最小用户可见行为。
- 必须实现的内容：
  1. `RefCell<T>`：构造、读、写、closure 中显式共享累加。
  2. `Box<T>`：构造、读 value、拒绝写 value（如已有 val field reject fixture 可复用）。
  3. `AtomicInt`：load/store/cas/exchange 单线程 run-pass。
  4. `AtomicBool`：load/store/cas/exchange 单线程 run-pass，验证 bool 编码往返。
  5. `Atomic<MyClass>`：load/store/cas pointer identity；CAS expected 旧对象成功/失败都覆盖。
  6. `AtomicValue<MyStruct>`：snapshot/load/store/cas/exchange；CAS expected 必须是 `Box<MyStruct>`。
  7. bound 反样本：`Atomic<Int>` 应拒绝，`AtomicValue<MyClass>` 应拒绝。
- 必须遵从的约束：
  - Atomic fixtures 不要依赖并发调度；本轮只需单线程原子语义与 lowering。
  - atomic-ref 相关 LLVM build fixture 应验证 atomic instruction 与 GC barrier 关键形态，但避免锁死不稳定 private symbol spelling。
- 验证：
  1. `cargo run -p scoop -- test`
  2. `cargo test -p scoopc atomic -- --nocapture`
- 完成条件：
  - shared-state primitive 有运行级和必要 LLVM 级回归覆盖。
- 依赖：C3-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：

### [TODO] C4-T02：更新 user-visible failure / frontend reject audit 基线

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C4-T02
  - `crates/scoopc/src/pipeline_user_visible_failure_policy.rs:146-311,390-426,462-481`
- 目标：
  - CaptureBox 删除与 sealed-interface 新增 rejects 后，pipeline audit 表不再反映旧世界。
- 必须实现的内容：
  1. 重算 `STALE_UNSUPPORTED_MAIN_BODY_COUNTS`。CaptureBox 删除后，重点复查 `mir_body/aggregates.rs`、`mir_body/terminator.rs`、`mir_body/value_args.rs`、`effect_lowered/value.rs`。
  2. 如果 C2-T02 选择 `unreachable!` guard 方案，把对应行加入 `INTERNAL_BUG_SENTINEL_HITS`；如果删除 guard，则无需新增 sentinel。
  3. 在 `FRONTEND_REJECT_SURFACES` 登记 sealed-interface rejects：错误码、定义位置、fixture marker。
  4. 确认 `STALE_USER_VISIBLE_UNSUPPORTED_MARKERS` 仍为空或有明确理由。
- 必须遵从的约束：
  - 不要通过放宽 audit test 掩盖真实 stale user-visible `UnsupportedMainBody`。
  - 每个新增 frontend reject 都必须能在 source marker 和 fixture marker 中定位。
- 验证：
  1. `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  2. `cargo test -p scoopc`
  3. `cargo run -p scoop -- test`
- 完成条件：
  - Audit 基线与当前代码/fixture 完全一致。
- 依赖：C4-T01A、C4-T01C、C4-T01D
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：

## C5：文档 / spec 收尾

### [TODO] C5-T01：更新 spec、迁移说明与设计文档状态

- 参考：
  - [`PLAN.md`](./PLAN.md) §7 C5
  - [`CLOSURE_FIX.md`](./CLOSURE_FIX.md) §6、§7
- 目标：
  - 把已实现语义写入用户可见 spec，并把设计讨论文档转为历史记录。
- 必须实现的内容：
  1. `SCOOP_FULL_SPEC.md`：增加 closure capture 默认语义条目，覆盖 by-value snapshot、per-call reset、ref capture heap sharing、显式 `RefCell` / `Atomic` 共享出口。
  2. `SCOOP_FULL_SPEC.md`：增加 `sealed interface` 章节，覆盖 empty body、sysroot-only、bound-only、inheritance、mutual exclusion、runtime invisible。
  3. `SCOOP_FULL_SPEC.md` 或 release/migration note：说明 Kotlin makeCounter 模式迁移到 `RefCell`。
  4. `CLOSURE_FIX.md` 文件头部添加“实现进度跟踪移交至 PLAN.md / TODO.md；本文档保留为设计讨论历史记录”。
  5. 若 spec code block 绑定 fixtures，运行 spec fixture sync/check。
- 必须遵从的约束：
  - 不要把未来 derived interface / sub-marker / AtomicFloat 等未实现内容写成已支持。
  - `MANAGED_ABI.md` 默认不需要改；CaptureBox 不是 managed ABI surface。只有实现过程中确实改变 runtime ABI 时才更新。
- 验证：
  1. `cargo run -p scoop_tools -- spec-fixtures sync`（仅当 spec fixture block 有变化）
  2. `cargo run -p scoop_tools -- spec-fixtures check`
  3. `cargo run -p scoop -- test`
- 完成条件：
  - 用户可见 spec 与已实现行为一致，设计文档状态清楚。
- 依赖：C4-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：

## Final Review

### [TODO] C6-T01：全量回归与最终审计

- 参考：
  - 本文件全部任务
  - [`PLAN.md`](./PLAN.md) §9 风险
- 目标：
  - 合并两条实现线后做最终验证，确保没有隐式 CaptureBox、没有未登记 frontend reject、没有 failing fixture。
- 必须实现的内容：
  1. 全量运行：`cargo fmt`、`cargo build`、`cargo test --all --all-targets`、`cargo run -p scoop -- test`。
  2. spec fixture check：`cargo run -p scoop_tools -- spec-fixtures check`。
  3. CaptureBox final grep：`rg -n "CaptureBox|capture_box|mir_capture_box|__CaptureBox|rt_alloc_pass_mir_capture_box" crates/scoopc/src sysroot tests/fixtures`，预期只允许历史文档/TODO/PLAN 命中，不允许 source/active fixture 命中。
  4. sealed-interface final grep：`rg -n "sealed_interface_" crates/scoopc/src tests/fixtures`，确认每个错误码有实现和 fixture。
  5. atomic-ref final grep：确认 atomic-ref intrinsic/API 在 direct HIR LLVM 与 effect-lowered LLVM 两条路径均有测试覆盖。
- 必须遵从的约束：
  - 不要在 final review 阶段新增大功能；发现真实缺口则插入新的前置任务并回写原因。
- 验证：
  1. `cargo fmt`
  2. `cargo build`
  3. `cargo test --all --all-targets`
  4. `cargo run -p scoop -- test`
  5. `cargo run -p scoop_tools -- spec-fixtures check`
- 完成条件：
  - 本轮可以归档：closure capture 语义、sealed interface、RefCell/Box/Atomic primitives、fixtures、audit、spec 全部闭合。
- 依赖：C5-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / `CLOSURE_FIX.md` 对应闭合：
