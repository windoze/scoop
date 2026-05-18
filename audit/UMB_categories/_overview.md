# UMB Bucket Overview

生成时间：2026-05-18  
inventory 来源：[`audit/UMB_inventory.csv`](../UMB_inventory.csv)  
生成方式：U2-T01 从当前 inventory 汇总；本文件不修改 production codegen。

## Summary

- Total active inventory entries: 1,053
- Bucket range: B-01 through B-36
- Zero-entry buckets: B-01, B-02, B-04, B-15, B-16, B-36
- Missing `spec_anchor`: 0
- Missing `upstream_gate`: 0
- Decision: all 36 stable buckets are retained; no merge or split is required in U2-T01.

## Bucket Table

| bucket | 名称 | 一级类 | entry 数 | expected_class 分布 | 主要 kind 标签前 5 条 | 主要文件前 5 个 | 备注 |
|---|---|---:|---:|---|---|---|---|
| B-01 | inkwell builder bookkeeping | A | 0 | retired InternalBugSentinel=71 | retired: builder has no insert block (29)<br>builder has no parent function (23)<br>function has no entry block (3)<br>atomicRefCompareExchange insert block (2)<br>atomicRefCompareExchange parent function (2) | retired ledger: `audit/UMB_retired.csv` | P7-B1 retired; active CSV empty. |
| B-02 | MIR local / member 类型推断不完整 | B | 0 | retired InternalBugSentinel=6 | retired: unknown local value (1)<br>local slot gep source type (1)<br>val without initializer (1)<br>MIR unresolved name source type (1)<br>pass MIR local member field type drift (1) | retired ledger: `audit/UMB_retired.csv` | P7-B2.1 retired; active CSV empty. |
| B-03 | MIR direct/closure/funptr 调用 ABI 漂移 | B | 56 | InternalBugSentinel=56 | pass MIR call arg binding (5)<br>itable direct result storage (2)<br>pass MIR call arg type (2)<br>call arg value (1)<br>call arg type (1) | crates/scoopc/src/llvm/codegen/call/lowering.rs (23)<br>crates/scoopc/src/llvm/codegen/mir_body/call.rs (14)<br>crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs (9)<br>crates/scoopc/src/llvm/codegen/mir_body/args.rs (6)<br>crates/scoopc/src/llvm/codegen/call/abi.rs (3) | CSV non-empty; body pending U2-T02. |
| B-04 | MIR 函数签名 / 参数 / 返回类型缺失 | B | 0 | retired InternalBugSentinel=29 | retired: function return type (6)<br>scoop_alloc_typed return type (6)<br>function param type (4)<br>DYNAMIC:kind (2)<br>scoop_alloc_typed enum box return type (1) | retired ledger: `audit/UMB_retired.csv` | P7-B2.1 retired; active CSV empty. |
| B-05 | MIR CFG / start block / goto target 异常 | B | 25 | InternalBugSentinel=25 | DYNAMIC:kind (2)<br>pass MIR param local (2)<br>interpolated string after HIR desugar (2)<br>if output type (1)<br>if condition value (1) | crates/scoopc/src/llvm/codegen/mir_body/terminator.rs (13)<br>crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs (7)<br>crates/scoopc/src/llvm/codegen/control_flow.rs (5) | CSV non-empty; body pending U2-T02. |
| B-06 | MIR struct/tuple/enum 字面量 schema 漂移 | B | 43 | InternalBugSentinel=43 | tuple type id (8)<br>tuple element type (3)<br>struct literal type (2)<br>tuple literal type (2)<br>pass MIR tuple element type (2) | crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs (21)<br>crates/scoopc/src/llvm/codegen/main/expr_value.rs (8)<br>crates/scoopc/src/llvm/codegen/layout.rs (4)<br>crates/scoopc/src/llvm/codegen/ty.rs (4)<br>crates/scoopc/src/llvm/codegen/control_flow.rs (3) | CSV non-empty; body pending U2-T02. |
| B-07 | MIR pattern 子句 schema 漂移 | B | 34 | InternalBugSentinel=34 | pass MIR pattern is target type (2)<br>pass MIR char literal (1)<br>pass MIR int builtin type (1)<br>pass MIR synthesized int builtin type (1)<br>pass MIR pattern int subject (1) | crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs (34) | CSV non-empty; body pending U2-T02. |
| B-08 | MIR 成员存取 / 赋值合法性 | B | 4 | InternalBugSentinel=4; retired FrontendReject=2 | pass MIR member store value type (1)<br>pass MIR member store operand type (1)<br>pass MIR member store receiver place (1)<br>pass MIR packed struct member store (1) | crates/scoopc/src/llvm/codegen/mir_body/member.rs (4) | P7-A2 retired frontend writability rows; internal rows remain active. |
| B-09 | Cross-TypeStore equivalence 不闭合 | B/C | 13 | InternalBugSentinel=13 | funptr local cg type (1)<br>funptr top-level cg type (1)<br>funptr carrier cg type (1)<br>task transport tuple first element codegen type (1)<br>task transport tuple second element codegen type (1) | crates/scoopc/src/llvm/codegen/mir_body/cast.rs (3)<br>crates/scoopc/src/llvm/codegen/call/lowering.rs (2)<br>crates/scoopc/src/llvm/codegen/effect_outcome.rs (2)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/call_invoke.rs (1)<br>crates/scoopc/src/llvm/codegen/intrinsics/named.rs (1) | CSV non-empty; cross-class split remains entry-level. |
| B-10 | Effect-typed callable adapter / ABI routing | C | 109 | RealImpl=109 | missing incoming_resume_token_ref param (1)<br>missing effect outcome param (1)<br>itable effect outcome slot (1)<br>Continuation.resume lowering (1)<br>funptr carrier source type (1) | crates/scoopc/src/llvm/codegen/effect_lowered/value.rs (63)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs (9)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/main_carrier.rs (5)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/states.rs (5)<br>crates/scoopc/src/llvm/codegen/mir_body/call.rs (5) | CSV non-empty; body pending U2-T02. |
| B-11 | Pure / plain statement 边界路由 | B | 14 | InternalBugSentinel=14 | assignment lhs (2)<br>anonymous val binding (1)<br>val type (1)<br>val without initializer (1)<br>unknown local value (1) | crates/scoopc/src/llvm/codegen/stmt.rs (14) | CSV non-empty; body pending U2-T02. |
| B-12 | Closure / lambda / capture 表达 | C | 50 | RealImpl=50 | capture local not found (3)<br>capture local type (3)<br>capture local (non-scalar) (3)<br>lambda return type (2)<br>capture type (2) | crates/scoopc/src/llvm/codegen/closure/mod.rs (19)<br>crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs (11)<br>crates/scoopc/src/llvm/codegen/mir_body/operand.rs (6)<br>crates/scoopc/src/llvm/codegen/main/identity.rs (4)<br>crates/scoopc/src/llvm/codegen/mir_body/call.rs (3) | CSV non-empty; body pending U2-T02. |
| B-13 | 数组 / 复合 transport metadata | C | 24 | RealImpl=24 | composed call replay block (1)<br>task transport resume payload type (1)<br>task transport operand source type (1)<br>task transport tuple value (1)<br>task transport tuple type (1) | crates/scoopc/src/llvm/codegen/effect_outcome.rs (10)<br>crates/scoopc/src/llvm/codegen/intrinsics/named.rs (7)<br>crates/scoopc/src/llvm/codegen/main/runtime_error.rs (2)<br>crates/scoopc/src/llvm/codegen/mir_body/transport.rs (2)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/composed_call.rs (1) | CSV non-empty; body pending U2-T02. |
| B-14 | Cast / TypeCheck (`as`/`as?`/`is`) | B/C | 27 | InternalBugSentinel=27 | type check operand (ref) (1)<br>type check operand value (1)<br>type check operand type (1)<br>as? result type (Option<T>) (1)<br>as? target (ref) (1) | crates/scoopc/src/llvm/codegen/mir_body/cast.rs (13)<br>crates/scoopc/src/llvm/codegen/main/expr_op.rs (12)<br>crates/scoopc/src/llvm/codegen/main/gc_locals.rs (2) | CSV non-empty; cross-class split remains entry-level. |
| B-15 | When / 模式匹配用户面 | B | 0 | retired FrontendReject=55 | retired: when unknown enum variant (3)<br>when variant payload field index (3)<br>when arm type mismatch (2)<br>when arm tail block (2)<br>when pattern (enum) (2) | retired ledger: `audit/UMB_retired.csv` | P7-A3 retired; active CSV empty. |
| B-16 | 控制流 outside-of-context | B | 0 | retired FrontendReject=7 | retired: break outside loop (3)<br>continue outside loop (3)<br>return outside function with return context (1) | retired ledger: `audit/UMB_retired.csv` | P7-A1 retired; active CSV empty. |
| B-17 | Coercion / 标量运算 | A/B | 47 | InternalBugSentinel=47 | equality lhs string value (2)<br>equality lhs string type (2)<br>equality rhs string value (2)<br>equality rhs string type (2)<br>String equals return value (2) | crates/scoopc/src/llvm/codegen/main/coerce.rs (31)<br>crates/scoopc/src/llvm/codegen/main/expr_op.rs (7)<br>crates/scoopc/src/llvm/codegen/effect_outcome.rs (5)<br>crates/scoopc/src/llvm/codegen/expr.rs (4) | CSV non-empty; cross-class split remains entry-level. |
| B-18 | 字面量与字符串 | B | 4 | InternalBugSentinel=4 | source-backed literal span (1)<br>source-backed literal slice (1)<br>int literal type (1)<br>scoop_alloc_typed return value (1) | crates/scoopc/src/llvm/codegen/main/context.rs (2)<br>crates/scoopc/src/llvm/codegen/main/literal.rs (2) | CSV non-empty; body pending U2-T02. |
| B-19 | Top-level / object init / extern global | B | 39 | InternalBugSentinel=39 | top-level immutable value type (3)<br>top-level var type (2)<br>object property access (missing metadata) (2)<br>object property type (2)<br>function value top-level type (1) | crates/scoopc/src/llvm/codegen/main/immut_value.rs (13)<br>crates/scoopc/src/llvm/codegen/object_init.rs (13)<br>crates/scoopc/src/llvm/codegen/main/globals.rs (9)<br>crates/scoopc/src/llvm/codegen/mir_body/member.rs (2)<br>crates/scoopc/src/llvm/codegen/call/lowering.rs (1) | CSV non-empty; body pending U2-T02. |
| B-20 | Class ctor / property / 字段访问 | B | 46 | InternalBugSentinel=46 | DYNAMIC:kind (11)<br>class ctor param type (6)<br>class field type (3)<br>class field index (2)<br>class field receiver value (2) | crates/scoopc/src/llvm/codegen/class_ctor.rs (30)<br>crates/scoopc/src/llvm/codegen/layout.rs (3)<br>crates/scoopc/src/llvm/codegen/main/call.rs (3)<br>crates/scoopc/src/llvm/codegen/mir_body/args.rs (3)<br>crates/scoopc/src/llvm/codegen/main/expr_value.rs (2) | CSV non-empty; body pending U2-T02. |
| B-21 | Struct literal / 字段层 | B | 3 | InternalBugSentinel=3; retired FrontendReject=3 | struct field value (1)<br>pass MIR class member field index (1)<br>struct field type (1) | crates/scoopc/src/llvm/codegen/main/expr_value.rs (1)<br>crates/scoopc/src/llvm/codegen/mir_body/member.rs (1)<br>crates/scoopc/src/llvm/codegen/ty.rs (1) | P7-A2 retired frontend unknown/missing-field rows; internal rows remain active. |
| B-22 | Enum 布局 / niche / Option | B | 40 | InternalBugSentinel=40 | Option niche pointer none_value (must be NULL) (3)<br>unknown enum variant (3)<br>enum variant ctor arity mismatch (2)<br>enum payload string (2)<br>enum payload ref (2) | crates/scoopc/src/llvm/codegen/enum_lowering.rs (29)<br>crates/scoopc/src/llvm/codegen/control_flow.rs (4)<br>crates/scoopc/src/llvm/codegen/mir_body/member.rs (3)<br>crates/scoopc/src/llvm/codegen/layout.rs (2)<br>crates/scoopc/src/llvm/codegen/call/lowering.rs (1) | CSV non-empty; body pending U2-T02. |
| B-23 | Member access - 通用 | B | 24 | InternalBugSentinel=24 | pass MIR member receiver type (3)<br>member access receiver type (2)<br>member access receiver value (2)<br>member access target (2)<br>itable value-box receiver type (1) | crates/scoopc/src/llvm/codegen/mir_body/member.rs (7)<br>crates/scoopc/src/llvm/codegen/main/expr_value.rs (6)<br>crates/scoopc/src/llvm/codegen/call/lowering.rs (4)<br>crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs (3)<br>crates/scoopc/src/llvm/codegen/mir_body/mod.rs (2) | CSV non-empty; body pending U2-T02. |
| B-24 | Reflection / comptime intrinsic | C/D | 6 | RealImpl=6 | sizeOf() arity mismatch (1)<br>sizeOf() arg type (1)<br>reflection intrinsic call binding (1)<br>pass MIR sizeOf arg type (1)<br>pass MIR kindOf arg type (1) | crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs (3)<br>crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs (3) | CSV non-empty; cross-class split remains entry-level. |
| B-25 | Platform / RTTI intrinsic | B/C | 14 | RealImpl=14 | MIR runtime type-check metadata (1)<br>MIR `as?` target runtime type (1)<br>MIR runtime type descriptor (1)<br>MIR runtime type target (1)<br>MIR runtime type operand (1) | crates/scoopc/src/llvm/codegen/mir_body/cast.rs (6)<br>crates/scoopc/src/llvm/codegen/mir_body/transport.rs (6)<br>crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs (2) | CSV non-empty; cross-class split remains entry-level. |
| B-26 | atomic intrinsic 系列 | B | 102 | InternalBugSentinel=102 | atomicInt target type (5)<br>atomicInt extern global type (4)<br>atomicInt requires mutable lvalue (3)<br>atomicInt target width (3)<br>atomicRef requires mutable lvalue (3) | crates/scoopc/src/llvm/codegen/intrinsics/atomic.rs (49)<br>crates/scoopc/src/llvm/codegen/effect_lowered/value.rs (45)<br>crates/scoopc/src/llvm/codegen/main/call.rs (8) | CSV non-empty; body pending U2-T02. |
| B-27 | sync intrinsic 系列 | B | 58 | InternalBugSentinel=58 | sync.Once.isDone return value (2)<br>sync.Once.isDone return type (2)<br>sync.destroy receiver nominal (2)<br>sync.destroy receiver source type (1)<br>sync.destroy receiver nominal type (1) | crates/scoopc/src/llvm/codegen/intrinsics/sync.rs (53)<br>crates/scoopc/src/llvm/codegen/effect_lowered/value.rs (5) | CSV non-empty; body pending U2-T02. |
| B-28 | thread intrinsic 系列 | B | 20 | InternalBugSentinel=20 | thread.currentId return value (2)<br>thread.currentId return type (2)<br>thread.sleepMillis value (1)<br>thread.threadSpawn arity mismatch (1)<br>thread.threadSpawn named arg (block) (1) | crates/scoopc/src/llvm/codegen/intrinsics/thread.rs (17)<br>crates/scoopc/src/llvm/codegen/effect_lowered/value.rs (3) | CSV non-empty; body pending U2-T02. |
| B-29 | GC intrinsic 系列 | B | 93 | InternalBugSentinel=93 | GC.pin arg value (2)<br>GC.pin return type (2)<br>GC.pin field value (2)<br>GC.unpin arg type (2)<br>GC.unpin arg value (2) | crates/scoopc/src/llvm/codegen/gc.rs (56)<br>crates/scoopc/src/llvm/codegen/mir_body/member.rs (21)<br>crates/scoopc/src/llvm/codegen/effect_lowered/value.rs (10)<br>crates/scoopc/src/llvm/codegen/intrinsics/named.rs (2)<br>crates/scoopc/src/llvm/codegen/main/gc_locals.rs (2) | CSV non-empty; body pending U2-T02. |
| B-30 | named / unsafe / FunPtr intrinsic | B | 117 | InternalBugSentinel=117 | DYNAMIC:kind (11)<br>funptr signature type (2)<br>funptr signature kind (2)<br>stackmap statepoint smoke arity mismatch (2)<br>stackmap statepoint smoke caller function (2) | crates/scoopc/src/llvm/codegen/intrinsics/named.rs (43)<br>crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs (33)<br>crates/scoopc/src/llvm/codegen/intrinsics/sysroot.rs (15)<br>crates/scoopc/src/llvm/codegen/call/lowering.rs (7)<br>crates/scoopc/src/llvm/codegen/mir_body/call.rs (5) | CSV non-empty; body pending U2-T02. |
| B-31 | 标量扩展方法 (Float/Int/Char/Bool/String) | B | 12 | InternalBugSentinel=12 | String.byteLength arity mismatch (1)<br>String.byteLength load type (1)<br>String.getByte arity mismatch (1)<br>String.getByte named arg (1)<br>String.getByte index value (1) | crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs (12) | CSV non-empty; body pending U2-T02. |
| B-32 | print / panic / sysroot 桥接 | B | 10 | InternalBugSentinel=10 | sysroot panic arity mismatch (1)<br>sysroot panic named arg (1)<br>sysroot panic message value (1)<br>sysroot panic message type (1)<br>sysroot print/println String arg value (1) | crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs (10) | CSV non-empty; body pending U2-T02. |
| B-33 | Extern global / FunPtr 顶层 | B | 8 | InternalBugSentinel=8 | extern global type (3)<br>pass MIR extern global store target immutable (2)<br>pass MIR extern global store target type (2)<br>extern global initializer contract (1) | crates/scoopc/src/llvm/codegen/mir_body/member.rs (4)<br>crates/scoopc/src/llvm/codegen/main/globals.rs (2)<br>crates/scoopc/src/llvm/codegen/main/immut_value.rs (2) | CSV non-empty; body pending U2-T02. |
| B-34 | RuntimeError / try-catch-finally | B | 6 | InternalBugSentinel=6 | completion payload target type (1)<br>RuntimeError enum layout (1)<br>RuntimeError variant layout (1)<br>RuntimeError payload variant arity (1)<br>RuntimeError unit variant value (1) | crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_error.rs (3)<br>crates/scoopc/src/llvm/codegen/effect_lowered/body/payload.rs (1)<br>crates/scoopc/src/llvm/codegen/main/runtime_error.rs (1)<br>crates/scoopc/src/llvm/codegen/mir_body/cast.rs (1) | CSV non-empty; body pending U2-T02. |
| B-35 | unsafe / NoGC / 边界 | B/C | 5 | InternalBugSentinel=5 | spill slot/frame slot count mismatch (2)<br>spill slot explicit frame mirrors (1)<br>value/frame slot count mismatch (1)<br>value primitive boundary payload requires published contract (1) | crates/scoopc/src/llvm/codegen/main/declare.rs (2)<br>crates/scoopc/src/llvm/codegen/main/frame.rs (2)<br>crates/scoopc/src/llvm/codegen/mir_body/terminator.rs (1) | CSV non-empty; cross-class split remains entry-level. |
| B-36 | 未定义/暂未支持的 spec surface | D | 0 | retired FrontendReject=58 | retired: DYNAMIC:kind (8)<br>scoop_alloc_typed return value (4)<br>struct type id (4)<br>enum type id (4)<br>DYNAMIC:missing_kind (2) | retired ledger: `audit/UMB_retired.csv` | P7-A4 retired; async/await and generator/yield remain frontend rejects. |

## B-01 Skeleton Sample

This sample is retained as a historical shape reference; the live B-01 category is retired by P7-B1.

# B-01 - inkwell builder bookkeeping

本 bucket entry 数：0
inventory 来源：audit/UMB_inventory.csv  
生成时间：2026-05-18  
负责人/状态：P7-B1 / retired

## Symptom

- Active summary: 0 entries; retired ledger covers the initial 71 helper-invariant IDs.

## Root Cause Hypothesis

- LLVM insertion-context invariants are centralized in `MainCodegen::expect_*` helpers.

## Spec Linkage

- Helper invariant entries use `N/A:helper-invariant`; U2-T02 should explain why spec linkage is not applicable.

## Expected Post-Fix Class

- Current distribution from inventory: all classes 0 active; retired `InternalBugSentinel=71`.

## Fix Strategy Outline

- P7-B1 migrated former B-01 `UnsupportedMainBody` sites to helper panic boundaries.

## Fixture Set Pointer

- Fixture/sentinel path: `tests/fixtures/umb_fix/B-01-builder-invariant/`.

## Open Questions

- No active B-01 blocker remains.
