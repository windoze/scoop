//! Late lowering / explicit root descriptor / managed function tests: state-machine carrier emission, explicit-root frame layout, function-value / closure-value, GC barriers.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn managed_function_emits_explicit_root_frame_tls_lifecycle_and_slot_clear() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

fun keep(name: String): String {
    __scoop_gc_collect()
    return name
}

fun main() {
    println(keep("hi"))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let keep_ir = function_ir_matching(
        &ir,
        "managed string helper with safepoint lifecycle",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_gc_collect")
                && function.contains("ret ptr addrspace(1)")
        },
    );

    assert!(
        ir.contains("@__scoop_explicit_root_frame_top = external thread_local global ptr"),
        "expected explicit root frame TLS declaration\n{ir}"
    );
    assert!(
        keep_ir.contains(
            "store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected function entry to push explicit root frame\n{keep_ir}"
    );
    assert!(
        keep_ir.contains(
            "store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected function return to restore previous explicit root frame\n{keep_ir}"
    );
    assert!(
        keep_ir.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_0")
            && !keep_ir.contains("load ptr addrspace(1), ptr %name"),
        "safepoint 之后应从 explicit frame home slot 重新读取 live root，而不是继续使用原局部 alloca\n{keep_ir}"
    );
    assert!(
        keep_ir.contains("store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0"),
        "expected function teardown to clear the explicit frame home slot back to NULL\n{keep_ir}"
    );
}

#[test]
pub(super) fn zero_slot_managed_function_still_emits_explicit_root_frame_lifecycle() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun label(n: Int): String {
    if (n == 0) {
        return "zero"
    }
    return "nonzero"
}

fun main() {
    println(label(0))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let label_ir = function_ir_matching(
        &ir,
        "zero-slot managed string helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && header.contains("ptr addrspace(1)")
                && !function.contains("@scoop_println")
        },
    );
    let label_desc = function_ir_explicit_root_descriptor(label_ir)
        .expect("zero-slot managed function should publish an explicit root descriptor");

    assert!(
        llvm_ir_defines_global(&ir, label_desc),
        "expected managed function to publish an explicit root descriptor\n{ir}"
    );
    assert!(
        label_ir.contains("%explicit_root_frame_storage = alloca ptr"),
        "expected managed function to allocate an explicit frame\n{label_ir}"
    );
    assert!(
        label_ir.contains(&format!(
            "store ptr @{label_desc}, ptr %explicit_root_frame_desc_ptr"
        )),
        "expected zero-slot managed function to publish its descriptor in the explicit frame header\n{label_ir}"
    );
    assert!(
        label_ir.contains(
            "store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected zero-slot managed function to push explicit root frame TLS\n{label_ir}"
    );
    assert!(
        label_ir.contains(
            "store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected zero-slot managed function to restore previous explicit root frame TLS on return\n{label_ir}"
    );
}

#[test]
pub(super) fn managed_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

fun keep(name: String): String {
    __scoop_gc_collect()
    return name
}

fun main() {
    println(keep("hi"))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let keep_ir = function_ir_matching(
        &ir,
        "managed string helper reloading direct GC local after safepoint",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_gc_collect")
                && function.contains("ret ptr addrspace(1)")
        },
    );
    let call_idx = keep_ir
        .find("@scoop_gc_collect")
        .expect("expected explicit safepoint helper call in keep() IR");
    let reload_window = &keep_ir[call_idx..];

    assert!(
        reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_0"),
        "post-safepoint direct local use should reload from explicit frame home slot\n{reload_window}"
    );
    assert!(
        !reload_window.contains("load ptr addrspace(1), ptr %name"),
        "post-safepoint direct local use should not fall back to the original local alloca\n{reload_window}"
    );
}

#[test]
pub(super) fn class_ctor_this_local_reloads_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

class Box(val name: String) {
    val copy: String = @Safe do {
        __scoop_gc_collect()
        this.name
    }
}

fun entry(): String {
    return Box("hi").copy
}

fun main() {
    println(entry())
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_matching(
        &ir,
        "top-level entry inlining ctor property initializer safepoint",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_gc_collect")
                && function.contains("rt_alloc_lowered_class")
        },
    );
    let call_idx = entry_ir
        .find("@scoop_gc_collect")
        .expect("expected ctor property initializer to emit a safepoint");
    let reload_window = &entry_ir[call_idx..];

    assert!(
        reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "ctor-inlined `this` should reload from explicit frame home slot after safepoint\n{reload_window}"
    );
    assert!(
        !reload_window.contains("load ptr addrspace(1), ptr %this"),
        "ctor-inlined `this` should not reload from the original local slot after safepoint\n{reload_window}"
    );
}

#[test]
pub(super) fn higher_order_aggregate_return_calls_sysroot_string_concat_helper() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

struct Labelled(val text: String, val score: Int)

fun main() {
    val mapper: (String) -> Labelled = { input: String ->
        val tagged = input.concat("!")
        Labelled { text: tagged, score: tagged.length() }
    }
    println(mapper("go").text)
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let lambda_ir = function_ir_matching(
        &ir,
        "closure body using sysroot concat in mapper",
        |header, function| {
            !header.contains("@main(")
                && function.contains("scoop_core_String_concat")
                && function.contains("explicit_root_frame_slot_0")
        },
    );
    assert!(
        !lambda_ir.contains("@scoop_string_concat"),
        "user closure should call the sysroot String.concat body instead of the runtime symbol directly\n{lambda_ir}"
    );

    let concat_ir = function_ir_matching(
        &ir,
        "compiled sysroot concat bridge helper",
        |_, function| {
            stable_id_symbol_mentions_fqn(
                llvm_function_symbol_name(function),
                "scoop.core.__scoop_string_concat",
            )
        },
    );
    assert!(
        concat_ir.contains("@scoop_string_concat(")
            && !concat_ir.contains("@scoop_enter_native")
            && !concat_ir.contains("@scoop_leave_native"),
        "compiled sysroot String.concat helper should bridge to runtime allocation substrate without native enter/leave\n{concat_ir}"
    );
}

#[test]
pub(super) fn class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

class Box(val name: String)

fun make(name: String): Box {
    return Box(f"{name}_boxed")
}

fun main() {
    println(make("hi").name)
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let make_ir = function_ir_matching(
        &ir,
        "class factory with GC-sensitive ctor arg evaluation",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && header.contains("ptr addrspace(1)")
                && !function.contains("@\"scoop.core.println::<String>\"")
                && function.contains("lowered_class_ctor_obj_root")
        },
    );
    let string_alloc_idx = make_ir
        .find("@__scoop_type_desc_runtime__ScoopString")
        .expect("expected ctor arg f-string allocation in make() IR");
    let reload_window = &make_ir[string_alloc_idx..];

    assert!(
        make_ir.contains(
            "store ptr addrspace(1) %rt_alloc_lowered_class, ptr %lowered_class_ctor_obj_root"
        ),
        "factory class ctor should spill the freshly allocated object before any GC-sensitive arg evaluation\n{make_ir}"
    );
    assert!(
        reload_window.contains("lowered_class_ctor_obj_before_invoke")
            && reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "ctor arg evaluation should reload the allocated object from its explicit-frame-backed root before invoking ctor init\n{reload_window}"
    );
    assert!(
        reload_window.contains("lowered_class_ctor_obj_return")
            && reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "factory return should reload the allocated object from its explicit-frame-backed root after ctor init\n{reload_window}"
    );
}

#[test]
pub(super) fn deferred_call_arg_reloads_from_explicit_frame_after_later_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

fun take(a: String, b: String): String {
    return a
}

fun later(): String {
    __scoop_gc_collect()
    return "b"
}

fun run(): String {
    return take("a", later())
}

fun main() {
    println(run())
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_matching(
        &ir,
        "run helper reloading deferred call arg after later safepoint",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && header.contains("ptr addrspace(1)")
                && function.contains(
                    "call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_",
                )
                && !function.contains("@\"scoop.core.println::<String>\"")
        },
    );
    let reload_idx = run_ir
        .find("call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_")
        .expect("expected deferred arg reload from explicit frame in run() IR");
    let take_idx = run_ir[reload_idx..]
        .find(" call ")
        .map(|idx| reload_idx + idx)
        .expect("expected deferred call after explicit-frame reload in run() IR");
    let reload_window_start = take_idx.saturating_sub(800);
    let reload_window = &run_ir[reload_window_start..take_idx + 200];

    assert!(
        run_ir
            .contains("call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "deferred GC call arg should rematerialize from explicit frame home slot after later safepoint\n{reload_window}"
    );
    assert!(
        !run_ir.contains("call_arg_reload_0 = load ptr addrspace(1), ptr %call_arg_0"),
        "deferred GC call arg should not reload from the original spill slot after later safepoint\n{reload_window}"
    );
    assert!(
        reload_window.contains("%pass_mir_call_arg_reload_0"),
        "deferred GC call should consume the explicit-frame reloaded argument, not the stale spill slot\n{reload_window}"
    );
}

#[test]
pub(super) fn aggregate_call_arg_rebuilds_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

struct Named(val name: String, val score: Int)

fun take(named: Named): String {
    return named.name
}

fun run(named: Named): String {
    __scoop_gc_collect()
    return take(named)
}

fun main() {
    println(run(Named { name: "hi", score: 1 }))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_matching(
        &ir,
        "aggregate call-arg rebuild helper after safepoint",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("call_arg_reload_0_rebuild = alloca")
        },
    );
    let frame_reload_idx = run_ir
        .find("call_arg_reload_0_frame_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_")
        .expect("expected aggregate arg frame reload before rebuilt call");
    let call_idx = run_ir[frame_reload_idx..]
        .find(" call ")
        .map(|idx| frame_reload_idx + idx)
        .expect("expected rebuilt aggregate call after explicit-frame reload");
    let reload_window_start = call_idx.saturating_sub(1600);
    let reload_window = &run_ir[reload_window_start..call_idx + 200];

    assert!(
        run_ir.contains("call_arg_reload_0_rebuild = alloca"),
        "aggregate call arg should rebuild a fresh by-value copy before the call\n{run_ir}"
    );
    assert!(
        reload_window.contains(
            "call_arg_reload_0_frame_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_"
        ),
        "aggregate call arg rebuild should reload GC leaf fields from explicit frame home slots\n{reload_window}"
    );
    assert!(
        reload_window.contains("%pass_mir_call_arg_reload_0_rebuild"),
        "aggregate call arg should pass the rebuilt slot instead of the stale original spill\n{reload_window}"
    );
}

#[test]
pub(super) fn hidden_sret_aggregate_result_rebuilds_from_explicit_frame_slots() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

struct Named(val name: String, val score: Int)

fun bounce(named: Named): Named {
    return named
}

fun run(named: Named): String {
    return bounce(named).name
}

fun main() {
    println(run(Named { name: "hi", score: 1 }))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_matching(
        &ir,
        "hidden-sret caller rebuilding aggregate result",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("call_sret_rebuild = alloca")
        },
    );
    let rebuild_idx = run_ir
        .find("call_sret_rebuild = alloca")
        .expect("expected hidden-sret rebuild slot in run() IR");
    let call_idx = run_ir[rebuild_idx..]
        .find(" call ")
        .map(|idx| rebuild_idx + idx)
        .expect("expected hidden-sret call after rebuild slot allocation");
    let reload_window = &run_ir[call_idx..std::cmp::min(call_idx + 1800, run_ir.len())];

    assert!(
        run_ir.contains("call_sret_rebuild = alloca"),
        "hidden sret aggregate result should rebuild a fresh aggregate slot before use\n{run_ir}"
    );
    assert!(
        reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "hidden sret aggregate result should reload GC leaf fields from explicit frame home slots\n{reload_window}"
    );
}

#[test]
pub(super) fn boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

struct Named(val name: String, val score: Int)

effect Ping {
    fun pong(value: Named): String
}

fun go(named: Named): String / Ping {
    __scoop_gc_collect()
    return Ping.pong(named)
}

fun main(): Int {
    val value = handle {
        go(Named { name: "hi", score: 1 })
    } with {
        Ping.pong(value: Named) -> value.name
    }
    return if (value == "hi") 0 else 1
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let go_ir = function_ir_matching(&ir, "managed outward payload helper", |header, function| {
        !header.contains("@main(")
            && header.contains("lowered_direct_invoke")
            && function.contains("outward_payload_reload_frame_reload")
    });
    let box_idx = go_ir
        .find("outward_payload_reload_frame_reload")
        .expect("expected outward payload reload in go() IR");
    let reload_window_start = box_idx.saturating_sub(1400);
    let reload_window = &go_ir[reload_window_start..std::cmp::min(box_idx + 400, go_ir.len())];

    assert!(
        go_ir.contains("outward_payload_reload_rebuild = alloca %a.Named")
            && go_ir.contains("outward_payload_reload_field_insert_0 = insertvalue %a.Named undef")
            && go_ir.contains("step_payload_insert = insertvalue")
            && go_ir.contains("%a.Named %outward_payload_reload, 0"),
        "outward payload should rebuild a fresh aggregate before publishing Step payload\n{go_ir}"
    );
    assert!(
        reload_window.contains("outward_payload_reload_frame_reload = load ptr addrspace(1)")
            && reload_window.contains(
                "outward_payload_reload_field_insert_0 = insertvalue %a.Named undef, ptr addrspace(1) %outward_payload_reload_frame_reload, 0"
            ),
        "outward payload rebuild should reload GC leaf fields from explicit frame home slots\n{reload_window}"
    );
}

#[test]
pub(super) fn never_returning_managed_function_pops_explicit_root_frame_before_unreachable() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.runtime.test.*

fun stop(name: String): Nothing / Raise<RuntimeError> {
    __scoop_gc_collect()
    Raise.raise(RuntimeError.NullAssertionFailed)
}

fun main(): Int {
    return try {
        stop("hi")
    } catch (e: RuntimeError) {
        1
    }
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let stop_ir =
        function_ir_matching(&ir, "never-returning managed helper", |header, function| {
            !header.contains("@main(")
                && function.contains("unreachable")
                && function.contains(
                    "store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top",
                )
                && function
                    .contains("store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0")
        });

    assert!(
        stop_ir.contains(
            "store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected never-returning function entry to push explicit root frame\n{stop_ir}"
    );
    assert!(
        stop_ir.contains("store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0"),
        "expected unreachable exit to clear explicit frame slots\n{stop_ir}"
    );
    assert!(
        stop_ir.contains(
            "store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top"
        ),
        "expected unreachable exit to restore previous explicit root frame\n{stop_ir}"
    );
}

#[test]
pub(super) fn nested_raise_try_catch_uses_innermost_handle_dispatch_contract() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1e_nested_raise_try_catch.scoop",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val r1: Int = try {
        println("try1")
        Raise.raise(10)
        println("unreachable1")
        0
    } catch (e: Int) {
        println("catch1")
        e
    }
    println(r1)

    val r2: Int = try {
        val inner: Int = try {
            println("inner_try")
            Raise.raise(20)
            println("unreachable_inner")
            0
        } catch (e: Int) {
            println("inner_catch")
            e
        }
        println("outer_try_after_inner")
        inner + 1
    } catch (e: Int) {
        println("outer_catch_unreachable")
        e
    }
    println(r2)

    println("done")
    return 0
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("define i32 @main("),
        "nested Raise.raise try/catch fixture should lower through EffectStep main codegen\n{ir}"
    );
}

#[test]
pub(super) fn effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1d_effect_step_tuple_param_carrier.scoop",
        r#"
package fixtures.t5000j1d

import scoop.core.*

enum MyOpt {
    Some(val value: Int),
    None,
}

fun explode(pair: (MyOpt, Int)): Unit / Raise<RuntimeError> {
    when (pair) {
        (Some(_), y) -> println(y)
        else -> Raise.raise(RuntimeError.NullAssertionFailed)
    }
}

fun main(): Int {
    val pair: (MyOpt, Int) = (None(), 7)
    val code: Int = try {
        explode(pair)
        99
    } catch (e: RuntimeError) {
        when (e) {
            NullAssertionFailed -> 0
            ClassCastFailed -> 1
            ContinuationAlreadyResumed -> 2
        }
    }
    return code
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let explode_ir = function_ir_matching(
        &ir,
        "tuple-shaped direct invoke helper for explode()",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_has_private_role(
                    llvm_function_symbol_name(function),
                    "lowered_direct_invoke",
                )
                && header.contains("({ %fixtures.t5000j1d.MyOpt, i64 } %0)")
        },
    );
    let explode_symbol = llvm_function_symbol_name(explode_ir);
    let explode_step_ty = llvm_function_return_named_struct_type(explode_ir)
        .expect("expected hashed Step return type for tuple-shaped direct invoke helper");

    assert!(
        stable_id_type_name_has_hashed_family(explode_step_ty, "Step")
            && ir.lines().any(|line| {
                line.contains(&format!("call %{explode_step_ty}"))
                    && llvm_line_mentions_symbol(line, explode_symbol)
                    && line.contains("({ %fixtures.t5000j1d.MyOpt, i64 }")
            }),
        "tuple-arg effect-step callable 应继续以 tuple-shaped direct invoke surface 传递参数，而不是回退到旧 wrapper/拆散 carrier\n{ir}"
    );
}

#[test]
pub(super) fn explicit_frame_layout_flattens_indirect_gc_aggregate_params() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

struct Named(val name: String, val score: Int)

fun first(named: Named): String {
    return named.name
}

fun main() {
    println(first(Named { name: "hi", score: 1 }))
}
"#,
    );
    let session = session_for_source(&source);
    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let ir = module.print_to_string().to_string();
    let first_ir = function_ir_matching(
        &ir,
        "indirect aggregate parameter helper with explicit frame",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && header.contains("ptr addrspace(1)")
                && function_ir_explicit_root_descriptor(function).is_some()
                && !function.contains(" call ")
                && function.contains("extractvalue")
        },
    );
    let first_desc = function_ir_explicit_root_descriptor(first_ir)
        .expect("aggregate parameter helper should publish an explicit root descriptor");
    let first_offsets = explicit_root_descriptor_offsets_symbol(&ir, first_desc)
        .expect("aggregate parameter helper descriptor should reference an offsets table");
    let frame_ty_name = format!(
        "scoop.runtime.ScoopExplicitRootFrame${}",
        llvm_sanitize_ident_for_test(llvm_function_symbol_name(first_ir))
    );

    let frame_ty = context
        .get_struct_type(&frame_ty_name)
        .expect("missing explicit frame type for aggregate parameter helper");
    assert_eq!(
        frame_ty.get_field_types().len(),
        3,
        "expected header + tracked aggregate/root leaf slots for Named.name"
    );
    assert!(
        llvm_ir_global_definition(&ir, first_offsets)
            .is_some_and(|line| line.contains("internal constant [2 x i32]")),
        "expected indirect aggregate param to publish tracked root slots\n{ir}"
    );
}

#[test]
pub(super) fn explicit_frame_layout_covers_hidden_sret_call_temps() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

struct Named(val name: String, val score: Int)

fun make(name: String): Named {
    return Named { name: name, score: 1 }
}

fun useIt(name: String): String {
    val named = make(name)
    return named.name
}

fun main() {
    println(useIt("hi"))
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let use_it_ir = function_ir_matching(
        &ir,
        "hidden-sret caller with explicit-frame call temps",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("call_sret_rebuild = alloca")
                && function_ir_explicit_root_descriptor(function).is_some()
        },
    );
    let use_it_desc = function_ir_explicit_root_descriptor(use_it_ir)
        .expect("hidden-sret caller should publish an explicit root descriptor");
    let use_it_offsets = explicit_root_descriptor_offsets_symbol(&ir, use_it_desc)
        .expect("hidden-sret caller descriptor should reference an offsets table");

    assert!(
        llvm_ir_defines_global(&ir, use_it_desc),
        "expected descriptor for hidden-sret caller\n{ir}"
    );
    assert!(
        llvm_ir_defines_global(&ir, use_it_offsets),
        "expected hidden-sret caller to emit root offsets table\n{ir}"
    );
}

#[test]
pub(super) fn top_level_immutable_init_emits_explicit_root_frame_descriptor() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

val greeting: String = "hi"

fun main() {
    println(greeting)
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let init_ir = function_ir_matching(
        &ir,
        "top-level immutable value init helper",
        |header, function| {
            !header.contains("@main(")
                && header.contains("top_level_val_init")
                && function_ir_explicit_root_descriptor(function).is_some()
        },
    );
    let init_symbol = llvm_function_symbol_name(init_ir);
    let init_desc = function_ir_explicit_root_descriptor(init_ir)
        .expect("top-level immutable init helper should store an explicit-root descriptor");

    assert!(
        stable_id_symbol_has_private_role(init_symbol, "top_level_val_init"),
        "top-level immutable init helper 应使用 top_level_val_init private role，实际符号: {init_symbol}"
    );
    assert!(
        llvm_ir_defines_global(&ir, init_desc),
        "expected top-level immutable initializer to emit a descriptor global\n{ir}"
    );
}

#[test]
pub(super) fn effect_state_machine_functions_emit_explicit_root_frame_descriptors() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ping {
    fun pong(value: Int): Int
}

fun go(): Int / Ping {
    return Ping.pong(7)
}

fun main(): Int {
    return handle {
        go()
    } with {
        Ping.pong(value: Int) -> value
    }
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let go_ir = function_ir_matching(
        &ir,
        "effectful callable entry helper",
        |header, function| {
            !header.contains("@main(")
                && function.contains("store i32 1")
                && function.contains("i64 7")
        },
    );
    let go_desc = function_ir_explicit_root_descriptor(go_ir)
        .expect("effectful callable entry should publish an explicit-root descriptor");

    assert!(
        llvm_ir_defines_global(&ir, go_desc),
        "effectful callable entry 应发布 direct-invoke descriptor global\n{ir}"
    );
    assert!(
        function_ir_count_matching(&ir, |header, function| {
            header.contains("surface_resume_owner_dispatch")
                && function_ir_explicit_root_descriptor(function).is_some()
        }) >= 1
            && function_ir_count_matching(&ir, |header, function| {
                header.contains("resume")
                    && !header.contains("surface_resume_owner_dispatch")
                    && function_ir_explicit_root_descriptor(function).is_some()
            }) >= 1,
        "effectful callable 的 resume/owner-dispatch 入口也应发布 explicit-root descriptors\n{ir}"
    );
}

#[test]
pub(super) fn plain_array_string_get_keeps_string_surface_for_println() {
    let source = SourceFile::new_virtual(
        "<mem>/t1703_string_array_println_surface.scoop",
        r#"
package fixtures.t1703

import scoop.core.*

fun printArray(label: String, xs: Array<String>): Unit {
    println(label)
    println(xs.get(0))
}

fun main() {
    val xs: Array<String> = ["alpha"]
    printArray("arr1:", xs)
}
"#,
    );
    let session = session_for_source(&source);
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_println"),
        "expected String println path to lower successfully\n{ir}"
    );
}

#[test]
pub(super) fn materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = session_for_source(&source);
    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let pass_view = codegen_unit
        .lowered
        .materialized_pass_view()
        .expect("production frontend 应保留 materialized pass view");
    let materialized_types = &pass_view.materialized().types;

    let mut seen_sites = 0usize;
    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            let Some(body) = &fun.body else {
                continue;
            };
            for block in &body.blocks {
                for stmt in &block.stmts {
                    let crate::mir::StatementKind::Assign {
                        value:
                            crate::mir::Rvalue::Call {
                                kind: crate::mir::CallKind::Direct { callee_fqn },
                                args,
                                ..
                            },
                        ..
                    } = &stmt.kind
                    else {
                        continue;
                    };
                    if callee_fqn != "scoop.core.println::<String>" {
                        continue;
                    }
                    let [arg] = args.as_slice() else {
                        continue;
                    };
                    let crate::mir::Operand::Local(local) = arg.value else {
                        continue;
                    };
                    let local_ty = body
                        .locals
                        .get(local.as_u32() as usize)
                        .expect("println::<String> arg local should exist")
                        .ty;
                    assert_eq!(
                        materialized_types.display(local_ty).to_string(),
                        "String",
                        "callable `{}` 应把 println::<String> 的 arg local{} 保持为 String surface",
                        fun.fqn,
                        local.as_u32()
                    );
                    seen_sites += 1;
                }
            }
        }
    }

    assert!(
        seen_sites > 0,
        "expected fixture to materialize at least one println::<String> call site"
    );
}

#[test]
pub(super) fn production_codegen_string_builder_fixture_materializes_mutable_array_push_instance() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/lang_string_builder_basic.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = session_for_source(&source);
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let pass_view = codegen_unit
        .lowered
        .materialized_pass_view()
        .expect("production frontend 应保留 materialized pass view");
    let mut pass_fun_fqns = Vec::new();
    for family in pass_view.instances() {
        pass_fun_fqns.extend(family.callable_bodies().map(|fun| fun.fqn.clone()));
    }
    assert!(
        pass_fun_fqns
            .iter()
            .any(|fqn| fqn.starts_with("scoop.core.push")),
        "expected string-builder fixture to materialize MutableArray.push instance in pass view, actual callables: {pass_fun_fqns:?}"
    );

    let mut seen_runtime_word_push = false;
    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            if !fun.fqn.starts_with("scoop.core.push") {
                continue;
            }
            let body = fun
                .body
                .as_ref()
                .expect("callable_bodies should only yield functions with bodies");
            for block in &body.blocks {
                for stmt in &block.stmts {
                    let crate::mir::StatementKind::Assign {
                        value:
                            crate::mir::Rvalue::Call {
                                kind: crate::mir::CallKind::Direct { callee_fqn },
                                transport,
                                ..
                            },
                        ..
                    } = &stmt.kind
                    else {
                        continue;
                    };
                    if callee_fqn != "scoop.core.__scoop_mutable_array_push_word" {
                        continue;
                    }
                    assert_eq!(
                        materialized
                            .types
                            .display(transport.result.source_ty)
                            .to_string(),
                        "Unit",
                        "callable `{}` 的 runtime MutableArray push 应保持 Unit result transport",
                        fun.fqn
                    );
                    seen_runtime_word_push = true;
                }
            }
        }
    }
    assert!(
        seen_runtime_word_push,
        "expected MutableArray.push body to call the runtime word push entry in pass view"
    );
}

#[test]
pub(super) fn production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = session_for_source(&source);
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend should retain materialized MIR");
    let pass_view = codegen_unit
        .lowered
        .materialized_pass_view()
        .expect("production frontend should retain materialized pass view");
    let mut main = None;
    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            if fun.fqn == "main" {
                main = Some(fun);
                break;
            }
        }
        if main.is_some() {
            break;
        }
    }
    let main = main.expect("expected literal numeric fixture to materialize main body");
    let body = main.body.as_ref().expect("main should have a body");

    let mut seen_uint8_pushes = 0;
    for block in &body.blocks {
        for stmt in &block.stmts {
            let crate::mir::StatementKind::Assign {
                value:
                    crate::mir::Rvalue::Call {
                        kind: crate::mir::CallKind::Direct { callee_fqn },
                        transport,
                        ..
                    },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            let callee_base = callee_fqn
                .rsplit_once("::<")
                .map(|(base, _)| base)
                .unwrap_or(callee_fqn.as_str());
            if callee_base != "scoop.core.push" {
                continue;
            }
            let array = transport
                .array
                .as_ref()
                .expect("MutableArray.push call should publish array transport metadata");
            if materialized.types.display(array.element_ty).to_string() != "UInt8" {
                continue;
            }
            assert_eq!(
                materialized
                    .types
                    .display(array.element.source_ty)
                    .to_string(),
                "UInt8",
                "main's UInt8 MutableArray.push should keep UInt8 source surface"
            );
            assert!(
                !array.element.requirements.trace,
                "main's UInt8 MutableArray.push should stay on scalar transport path"
            );
            assert!(
                !array.element.requirements.drop,
                "main's UInt8 MutableArray.push should not claim aggregate drop obligations"
            );
            assert!(
                array.element.boxing.is_none(),
                "main's UInt8 MutableArray.push should not publish composite boxing metadata"
            );
            seen_uint8_pushes += 1;
        }
    }

    assert_eq!(
        seen_uint8_pushes, 2,
        "expected the fixture's bytes array to retain two UInt8 MutableArray.push sites"
    );
}

pub(super) fn llvm_function_symbol_name(function_ir: &str) -> &str {
    let header = function_ir
        .lines()
        .next()
        .expect("expected function header");
    let symbol = header
        .split_once('@')
        .map(|(_, rest)| rest)
        .expect("expected function symbol name");
    if let Some(symbol) = symbol.strip_prefix('"') {
        symbol
            .split_once('"')
            .map(|(name, _)| name)
            .expect("expected closing quote in function symbol")
    } else {
        symbol
            .split_once('(')
            .map(|(name, _)| name)
            .expect("expected opening paren in function symbol")
    }
}

pub(super) fn llvm_function_return_named_struct_type(function_ir: &str) -> Option<&str> {
    let header = function_ir.lines().next()?;
    let before_symbol = header.split_once(" @")?.0;
    before_symbol.split_whitespace().last()?.strip_prefix('%')
}

pub(super) fn llvm_named_struct_name_matching<'ir, F>(
    ir: &'ir str,
    description: &str,
    predicate: F,
) -> &'ir str
where
    F: Fn(&str, &str) -> bool,
{
    let named_structs = ir
        .lines()
        .filter_map(|line| {
            let (name, _) = line.strip_prefix('%')?.split_once(" = type ")?;
            Some((name, line))
        })
        .collect::<Vec<_>>();
    named_structs
        .iter()
        .find_map(|(name, line)| predicate(name, line).then_some(*name))
        .unwrap_or_else(|| {
            let available = named_structs
                .iter()
                .map(|(name, _)| (*name).to_string())
                .collect::<Vec<_>>()
                .join("\n");
            panic!("expected named struct matching {description}; available structs:\n{available}")
        })
}

pub(super) fn stable_id_symbol_is_user_callable(symbol_name: &str) -> bool {
    symbol_name != "main"
        && !stable_id_symbol_looks_like_compiler_private_helper(symbol_name)
        && !stable_id_symbol_looks_like_runtime_or_native_import(symbol_name)
}

pub(super) fn llvm_sanitize_ident_for_test(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

pub(super) fn llvm_symbol_after_marker<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let symbol = text.split_once(marker)?.1;
    if let Some(symbol) = symbol.strip_prefix('"') {
        Some(
            symbol
                .split_once('"')
                .map(|(name, _)| name)
                .expect("expected closing quote in symbol reference"),
        )
    } else {
        let end = symbol.find([',', ' ', ')']).unwrap_or(symbol.len());
        Some(&symbol[..end])
    }
}

pub(super) fn llvm_line_mentions_symbol(line: &str, symbol_name: &str) -> bool {
    line.contains(&format!("@{symbol_name}")) || line.contains(&format!("@\"{symbol_name}\""))
}

pub(super) fn llvm_store_source_symbol(line: &str) -> Option<&str> {
    llvm_symbol_after_marker(line, "store ptr @")
}

pub(super) fn llvm_ir_stored_symbols_matching<F>(ir: &str, mut predicate: F) -> Vec<&str>
where
    F: FnMut(&str) -> bool,
{
    let mut symbols = Vec::new();
    for line in ir.lines() {
        let Some(symbol) = llvm_store_source_symbol(line) else {
            continue;
        };
        if predicate(symbol) && !symbols.contains(&symbol) {
            symbols.push(symbol);
        }
    }
    symbols
}

pub(super) fn llvm_load_gc_ref_from_global(line: &str) -> Option<(&str, &str)> {
    let (local, _) = line.split_once(" = load ptr addrspace(1), ptr @")?;
    let symbol = llvm_symbol_after_marker(line, " = load ptr addrspace(1), ptr @")?;
    Some((local.trim(), symbol))
}

pub(super) fn llvm_call_target_symbol(line: &str) -> Option<&str> {
    let after_call = if let Some(idx) = line.find(" call ") {
        &line[idx + " call ".len()..]
    } else if let Some(idx) = line.find(" invoke ") {
        &line[idx + " invoke ".len()..]
    } else {
        return None;
    };
    let symbol = after_call.split_once('@')?.1;
    if let Some(symbol) = symbol.strip_prefix('"') {
        Some(
            symbol
                .split_once('"')
                .map(|(name, _)| name)
                .expect("expected closing quote in call target symbol"),
        )
    } else {
        let end = symbol.find(['(', ' ', ',']).unwrap_or(symbol.len());
        Some(&symbol[..end])
    }
}

pub(super) fn function_ir_calls_symbol(function_ir: &str, symbol_name: &str) -> bool {
    function_ir
        .lines()
        .filter_map(llvm_call_target_symbol)
        .any(|callee| callee == symbol_name)
}

pub(super) fn function_ir_calls_matching_symbol<F>(function_ir: &str, predicate: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    function_ir
        .lines()
        .filter_map(llvm_call_target_symbol)
        .any(predicate)
}

pub(super) fn function_ir_explicit_root_descriptor(function_ir: &str) -> Option<&str> {
    const MARKER: &str = "store ptr @";
    const SLOT: &str = ", ptr %explicit_root_frame_desc_ptr";

    for line in function_ir.lines() {
        if !line.contains(SLOT) {
            continue;
        }
        let Some(start) = line.find(MARKER) else {
            continue;
        };
        let symbol = &line[start + MARKER.len()..];
        let Some((name, _)) = symbol.split_once(SLOT) else {
            continue;
        };
        return Some(name);
    }
    None
}

pub(super) fn llvm_ir_defines_global(ir: &str, symbol_name: &str) -> bool {
    ir.lines().any(|line| {
        line.starts_with(&format!("@{symbol_name} ="))
            || line.starts_with(&format!("@\"{symbol_name}\" ="))
    })
}

pub(super) fn llvm_ir_global_definition<'a>(ir: &'a str, symbol_name: &str) -> Option<&'a str> {
    ir.lines().find(|line| {
        line.starts_with(&format!("@{symbol_name} ="))
            || line.starts_with(&format!("@\"{symbol_name}\" ="))
    })
}

pub(super) fn explicit_root_descriptor_offsets_symbol<'a>(
    ir: &'a str,
    desc_symbol: &str,
) -> Option<&'a str> {
    let desc_line = llvm_ir_global_definition(ir, desc_symbol)?;
    let after_equals = desc_line
        .split_once("=")
        .map(|(_, rest)| rest)
        .expect("global definition should contain initializer");
    let (_, offset_ref) = after_equals.split_once(" ptr @")?;
    if let Some(offset_ref) = offset_ref.strip_prefix('"') {
        offset_ref.split_once('"').map(|(name, _)| name)
    } else {
        let end = offset_ref.find([',', ' ', '}']).unwrap_or(offset_ref.len());
        Some(&offset_ref[..end])
    }
}

pub(super) fn function_ir_count_matching<F>(ir: &str, predicate: F) -> usize
where
    F: Fn(&str, &str) -> bool,
{
    ir.split("\ndefine ")
        .skip(1)
        .filter(|chunk| {
            let end = chunk.find("\n}").expect("expected end of function body") + 2;
            let function = &chunk[..end];
            let header = function.lines().next().expect("expected function header");
            predicate(header, function)
        })
        .count()
}

pub(super) fn object_contains_stackmap_section(obj: &object::File<'_>) -> bool {
    obj.sections().any(|section| {
        section
            .name()
            .ok()
            .is_some_and(|name| name.contains("llvm_stackmaps"))
    })
}

pub(super) fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected: &str) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            let crate::mir::StatementKind::Assign {
                value:
                    crate::mir::Rvalue::Call {
                        kind: crate::mir::CallKind::Direct { callee_fqn },
                        ..
                    },
                ..
            } = &stmt.kind
            else {
                return false;
            };
            callee_fqn == expected
        })
    })
}

pub(super) fn mir_fun_contains_fun_value_call(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Call {
                        kind: crate::mir::CallKind::FunValue { .. },
                        ..
                    },
                    ..
                }
            )
        })
    })
}
