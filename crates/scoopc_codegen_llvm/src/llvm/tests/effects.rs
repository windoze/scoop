//! Effect / state-machine / continuation / escape / direct-call / closure tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn effect_contract_struct_types_are_registered_for_effect_codegen() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/effect_contract_types.scoop",
        r#"
package a

import scoop.core.*

struct Payload(val value: Int)

effect Ping {
    fun pong(value: Payload): Payload
}

fun go(): Payload / Ping {
    return Ping.pong(Payload(7))
}

fun main(): Int {
    val payload: Payload = handle {
        go()
    } with {
        Ping.pong(value: Payload) -> value
    }
    return payload.value
}
"#,
    );

    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();

    let composite_transport = context
        .get_struct_type("scoop.runtime.ScoopCompositeTransportDescriptor")
        .expect("effect codegen 应注册共享的 composite transport descriptor 类型");
    assert_eq!(composite_transport.count_fields(), 11);

    let ir = module.print_to_string().to_string();

    let step = context
        .get_struct_type(llvm_named_struct_name_matching(
            &ir,
            "hashed Step shell",
            |name, _| stable_id_type_name_has_hashed_family(name, "Step"),
        ))
        .expect("默认单文件 path 应为 outward callable 注册 hashed Step shell");
    assert_eq!(step.count_fields(), 2);

    let step_complete = context
        .get_struct_type(llvm_named_struct_name_matching(
            &ir,
            "hashed Step complete payload shell",
            |name, _| stable_id_type_name_has_hashed_family(name, "StepComplete"),
        ))
        .expect("Step 应发布 hashed complete payload shell");
    assert_eq!(step_complete.count_fields(), 1);

    let resume_vtable = context
        .get_struct_type(llvm_named_struct_name_matching(
            &ir,
            "hashed surface-resume vtable",
            |name, _| stable_id_type_name_has_hashed_family(name, "ResumeVtable"),
        ))
        .expect("continuation 应发布 authoritative hashed surface-resume vtable");
    assert_eq!(resume_vtable.count_fields(), 1);

    let continuation = context
        .get_struct_type(llvm_named_struct_name_matching(
            &ir,
            "hashed continuation object",
            |name, _| stable_id_type_name_has_hashed_family(name, "Continuation"),
        ))
        .expect("默认单文件 path 应为 handled perform 注册 hashed continuation object");
    assert!(
        continuation.count_fields() >= 10,
        "continuation object 至少应包含 header/resumed/state/effect_ctx/state_ref/step_fn/resume transport/captured suspend-state/vtable 字段"
    );

    assert!(
        !ir.contains("%scoop.lowered.Step__a_go")
            && !ir.contains("%scoop.lowered.StepComplete__a_go")
            && !ir.contains("%scoop.lowered.ResumeVtable__a_go__a_Ping")
            && !ir.contains("%scoop.lowered.Continuation__a_go")
            && ir.contains("surface_resume_owner_dispatch")
            && ir.contains("continuation_layout")
            && ir.contains("type_desc"),
        "默认单文件 path 应继续发布 surface-resume owner dispatch 与 continuation type descriptor 家族，而不是把旧 kN/type-desc 拼写写死在测试里:\n{ir}"
    );
}

#[test]
pub(super) fn indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport() {
    let source = SourceFile::new_virtual(
        "main.scoop",
        r#"
package a

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun go(): Int / Edge {
    return Edge.visit("left", 6)
}

fun main(): Int {
    return handle {
        go()
    } with {
        Edge.visit(from, to) -> to + 4
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("step_payload_insert") && ir.contains("switch i32 %step_tag"),
        "ordinary callee perform 应通过 Step payload/dispatch lower，而不是依赖旧 perform-slot runtime 入口\n{ir}"
    );
    assert!(
        ir.contains("= type { { ptr addrspace(1), i64 }, ptr addrspace(1) }")
            && ir.contains("insertvalue { ptr addrspace(1), i64 } undef")
            && ir.contains("insertvalue { ptr addrspace(1), i64 } %perform_payload_field0"),
        "multi-payload perform 应以内联 tuple payload 发布 Step case，而不是丢参或回旧 boxing ABI\n{ir}"
    );
    assert!(
        ir.contains("extractvalue { ptr addrspace(1), i64 } %boundary_case_payload_payload, 0")
            && ir.contains(
                "extractvalue { ptr addrspace(1), i64 } %boundary_case_payload_payload, 1"
            ),
        "handler binder lowering 应继续按 tuple payload 的两个字段读取 binder，而不是退回单值 transport\n{ir}"
    );
}

#[test]
pub(super) fn effectful_closure_dynamic_fallback_uses_schema_aware_carrier_adapter() {
    let source = SourceFile::new_virtual(
        "<mem>/closure_step_adapter.scoop",
        r#"
package a

import scoop.core.*

effect Ask {
    fun get(key: Int): Int
}

fun callIt(f: () -> Int / Ask): Int / Ask {
    f()
}

fun main(): Int {
    return handle {
        val x: Int = 10
        callIt {
            val y: Int = Ask.get(x)
            x + y
        }
    } with {
        Ask.get(key), k -> 99
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let stored_step_adapters = llvm_ir_stored_symbols_matching(&ir, |symbol| {
        stable_id_symbol_looks_like_compiler_private_helper(symbol)
            && stable_id_symbol_looks_like_closure_step_adapter_family(symbol)
    });
    assert_eq!(
        stored_step_adapters.len(),
        1,
        "effectful closure carrier 应只写入一个 schema-aware step adapter，而不是锁死某个当前拼写: {:?}\n{ir}",
        stored_step_adapters
    );
    let step_adapter_ir = maybe_function_ir_for_symbol(&ir, stored_step_adapters[0])
        .expect("stored schema-aware closure adapter should be defined in the same module");
    assert!(
        step_adapter_ir.contains("carrier_to_effectful"),
        "effectful closure carrier 应写入真正执行 carrier->effectful 转换的 adapter，而不是任意 private helper:\n{step_adapter_ir}"
    );
    assert!(
        !ir.lines()
            .filter_map(llvm_store_source_symbol)
            .any(stable_id_symbol_looks_like_closure_dynamic_entry_family),
        "closure surface step schema 与 owner step schema 不一致时，不应把 raw owner dynamic entry 直接写进 closure object:\n{ir}"
    );
}

#[test]
pub(super) fn higher_order_effectful_function_value_uses_schema_aware_carrier_adapter() {
    let source = SourceFile::new_virtual(
        "<mem>/higher_order_closure_step_adapter.scoop",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

enum Mode {
    Pure,
    Effectful(val seed: Int),
}

fun choose(mode: Mode): () -> Int / Ask {
    when (mode) {
        Pure -> {
            val thunk: () -> Int / Ask = { 5 }
            thunk
        }
        Effectful(seed) -> {
            val thunk: () -> Int / Ask = { Ask.ask(seed) }
            thunk
        }
    }
}

fun drive(mode: Mode): Int {
    return handle {
        choose(mode)()
    } with {
        Ask.ask(seed), k -> seed
    }
}

fun main(): Int {
    return drive(Effectful(9))
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let plain_adapters = llvm_ir_stored_symbols_matching(&ir, |symbol| {
        stable_id_symbol_looks_like_compiler_private_helper(symbol)
            && stable_id_symbol_looks_like_plain_adapter_family(symbol)
    });
    let step_adapters = llvm_ir_stored_symbols_matching(&ir, |symbol| {
        stable_id_symbol_looks_like_compiler_private_helper(symbol)
            && stable_id_symbol_looks_like_closure_step_adapter_family(symbol)
    });
    assert_eq!(
        plain_adapters.len(),
        1,
        "higher-order pure branch 应发布一个 plain adapter，而不是锁死当前符号拼写: {:?}\n{ir}",
        plain_adapters
    );
    assert_eq!(
        step_adapters.len(),
        1,
        "higher-order effectful branch 应发布一个 schema-aware carrier adapter，而不是锁死当前符号拼写: {:?}\n{ir}",
        step_adapters
    );
    let plain_adapter_ir = maybe_function_ir_for_symbol(&ir, plain_adapters[0])
        .expect("stored plain adapter should be defined in the same module");
    assert!(
        plain_adapter_ir.contains("carrier_to_plain"),
        "pure branch adapter 应执行 carrier->plain 转换，而不是依赖某个固定 helper 名字:\n{plain_adapter_ir}"
    );
    let step_adapter_ir = maybe_function_ir_for_symbol(&ir, step_adapters[0])
        .expect("stored effectful adapter should be defined in the same module");
    assert!(
        step_adapter_ir.contains("carrier_to_effectful"),
        "effectful branch adapter 应执行 carrier->effectful 转换，而不是依赖某个固定 helper 名字:\n{step_adapter_ir}"
    );
    assert!(
        !ir.lines()
            .filter_map(llvm_store_source_symbol)
            .any(stable_id_symbol_looks_like_closure_dynamic_entry_family),
        "effectful higher-order branch 不应把 raw owner dynamic entry 直接写进 closure object:\n{ir}"
    );
}

#[test]
pub(super) fn state_machine_multi_payload_perform_uses_tuple_transport() {
    let source = SourceFile::new_virtual(
        "main.scoop",
        r#"
package a

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun main(): Int {
    return handle {
        println("before")
        val x: Int = if (true) Edge.visit("left", 6) else 0
        println("after")
        x + 1
    } with {
        Edge.visit(from, to) , k -> {
            println(from)
            println(to)
            k.resume(to + 1)
        }
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("step_payload_insert") && ir.contains("switch i32 %step_tag"),
        "state-machine perform 应通过 Step payload/dispatch lower，而不是依赖旧 perform-slot runtime 入口\n{ir}"
    );
    assert!(
        ir.contains("= type { { ptr addrspace(1), i64 }, ptr addrspace(1) }")
            && ir.contains("insertvalue { ptr addrspace(1), i64 } undef")
            && ir.contains("insertvalue { ptr addrspace(1), i64 } %perform_payload_field0"),
        "state-machine multi-payload perform 应以内联 tuple payload 穿过 handle arm，而不是退回旧 boxing ABI\n{ir}"
    );
    assert!(
        ir.contains("handle_arm_payload_reload") && ir.contains("payload_field"),
        "state-machine handler binder lowering 应继续按 tuple payload 的两个字段读取 binder\n{ir}"
    );
    assert!(
        ir.contains("resume_step = call %scoop.lowered.Step")
            && ir.contains("surface_resume_owner_dispatch"),
        "Continuation.resume lowering 应改走 published surface-resume path，而不是旧 runtime helper 入口\n{ir}"
    );
    let resume_idx = ir
        .find("resume_step = call %scoop.lowered.Step")
        .expect("expected published surface-resume call in emitted IR");
    let resume_window_start = resume_idx.saturating_sub(500);
    let resume_window_end = std::cmp::min(resume_idx + 2200, ir.len());
    let resume_window = &ir[resume_window_start..resume_window_end];
    assert!(
        resume_window.contains("extractvalue %scoop.lowered.Step")
            && resume_window.contains("%resume_step, 0")
            && resume_window.contains("br i1 %step_is_complete"),
        "surface-resume call return path 应继续按 Step tag dispatch，而不是回答案专用 helper\n{resume_window}"
    );
    assert!(
        ir.contains("resume_state")
            && ir.contains("surface_resume_resumed_gep")
            && ir.contains("cmpxchg"),
        "surface-resume path 应继续显式消费 continuation state/cmpxchg one-shot contract\n{resume_window}"
    );
    assert!(
        ir.contains("store i32 %resume_state")
            || ir.contains("store i32 2, ptr addrspace(1) %cont_state_gep"),
        "surface-resume return path 应继续把 continuation state 写回 object contract\n{resume_window}"
    );
}

#[test]
pub(super) fn continuation_resume_driver_reloads_state_ref_from_explicit_frame_before_step_call() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../../tests/fixtures/run-pass/continuation_resume_answer_expression_basic.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let call_idx = ir
        .find("call void %cont_step_fn(")
        .expect("expected continuation step dispatch call in emitted IR");
    let window_start = call_idx.saturating_sub(900);
    let window_end = std::cmp::min(call_idx + 300, ir.len());
    let window = &ir[window_start..window_end];

    assert!(
        window.contains("load volatile ptr addrspace(1), ptr %explicit_root_frame_slot_8")
            && window.contains("call void %cont_step_fn(ptr addrspace(1) %frame_root_reload"),
        "continuation resume driver should reload state_ref from explicit-frame home slot before calling cont_step\n{window}"
    );
    assert!(
        !window.contains("call void %cont_step_fn(ptr addrspace(1) %load_frame_gc"),
        "continuation resume driver must not reuse stale pre-safepoint state_ref SSA at cont_step call site\n{window}"
    );
}

#[test]
pub(super) fn fresh_continuation_object_reloads_rooted_self_after_gc_barrier_init() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../../tests/fixtures/run-pass/continuation_resume_answer_expression_basic.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let state_ref_idx = ir
        .find("cont_state_ref_gep")
        .expect("expected continuation state_ref store in emitted IR");
    let window_start = state_ref_idx.saturating_sub(500);
    let window_end = std::cmp::min(state_ref_idx + 180, ir.len());
    let window = &ir[window_start..window_end];

    let barrier_idx = window
        .find("gc_write_barrier")
        .expect("expected write barrier before continuation state_ref init");
    let reload_idx = window[barrier_idx..]
        .find("cont_root_reload")
        .map(|idx| barrier_idx + idx)
        .expect("expected rooted continuation reload after write barrier");
    let state_ref_local_idx = window
        .find("cont_state_ref_gep")
        .expect("expected continuation state_ref GEP in local window");

    assert!(
        reload_idx < state_ref_local_idx
            && window[reload_idx..state_ref_local_idx]
                .contains("load volatile ptr addrspace(1), ptr %explicit_root_frame_slot_"),
        "fresh continuation object should reload rooted self after write-barrier init before storing state_ref\n{window}"
    );
}

#[test]
pub(super) fn composed_continuation_resume_publishes_internal_outcome_surface_and_owner_core() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../../tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("surface_resume_outcome")
            && ir.contains("surface_resume_owner_dispatch")
            && ir.contains("__outcome")
            && ir.contains("__core"),
        "composed continuation resume 应发布 internal outcome surface / owner outcome wrapper / owner core，而不是只剩 shared Step_F surface:\n{ir}"
    );

    let outcome_window = function_ir_matching(&ir, "owner outcome core", |header, _function| {
        llvm_function_header_uses_internal_or_private_linkage(header)
            && header.contains("surface_resume_owner__core")
    });
    assert!(
        outcome_window.contains("ScoopEffectOutcome")
            && outcome_window.contains("store")
            && !outcome_window.contains("call %scoop.lowered.Step"),
        "internal outcome surface 应直接写 explicit EffectOutcome，而不是继续把 Step_F 当 resume 内核\n{outcome_window}"
    );
}

#[test]
pub(super) fn composed_continuation_resume_reconstructs_step_from_internal_outcome_path() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../../tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("surface_resume_outcome") && ir.contains("composed_resume_outcome_phi"),
        "composed call-boundary resume 应先调用 internal outcome surface，再由 caller 侧重建 Step dispatch\n{ir}"
    );
    assert!(
        !ir.contains("composed_callee_resume = call %scoop.lowered.Step"),
        "composed call-boundary resume 不应再直接调用 shared Step_F surface 充当 resume 内核\n{ir}"
    );
}

#[test]
pub(super) fn cross_call_escape_resume_roots_do_not_degrade_to_poison_in_explicit_frame() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../../tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("explicit_root_frame_slot_"),
        "emitted IR should keep effect-lowered roots in explicit frame homes\n{ir}"
    );
    assert!(
        !ir.contains("ptr poison"),
        "cross-call escaped continuation resume roots must not degrade to poisoned spill homes\n{ir}"
    );
}

#[test]
pub(super) fn direct_effectful_signature_without_outward_effect_stays_on_direct_call_surface() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun hidden(): Int / (Ask) {
    return handle {
        Ask.ask(41)
    } with {
        Ask.ask(seed) -> seed + 1
    }
}

fun entry(): Int / (Ask) {
    return hidden()
}

fun main(): Int {
    return handle {
        entry()
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let hidden_ir = function_ir_matching(
        &ir,
        "handled effectful callee without outward Step dispatch",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && !function.contains("switch i32 %step_tag")
                && function.contains("i64 41")
        },
    );
    let hidden_symbol = llvm_function_symbol_name(hidden_ir);
    let entry_ir = function_ir_matching(
        &ir,
        "ordinary entry forwarding to handled effectful callee",
        |header, function| {
            !header.contains("@main(")
                && llvm_function_symbol_name(function) != hidden_symbol
                && !function.contains("switch i32 %step_tag")
                && function_ir_calls_symbol(function, hidden_symbol)
        },
    );

    assert!(
        function_ir_calls_symbol(entry_ir, hidden_symbol)
            && !entry_ir.contains("switch i32 %step_tag"),
        "签名 effectful 但 body 不 outward 的直调用应保持 direct-call surface，而不是进入 Step dispatch:\n{entry_ir}"
    );
}

#[test]
pub(super) fn direct_call_with_uncalled_effectful_higher_order_param_stays_on_direct_call_surface()
{
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun latent(thunk: () -> Int / (Ask)): Int / (Ask) {
    7
}

fun entry(): Int / (Ask) {
    return latent({ Ask.ask(5) })
}

fun main(): Int {
    return handle {
        entry()
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let latent_ir = function_ir_matching(
        &ir,
        "latent higher-order wrapper without outward Step dispatch",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && !function.contains("switch i32 %step_tag")
                && header.contains("ptr addrspace(1)")
                && !function.contains(" call ")
        },
    );
    let latent_symbol = llvm_function_symbol_name(latent_ir);
    let entry_ir = function_ir_matching(
        &ir,
        "ordinary entry forwarding latent higher-order parameter",
        |header, function| {
            !header.contains("@main(")
                && llvm_function_symbol_name(function) != latent_symbol
                && !function.contains("switch i32 %step_tag")
                && function_ir_calls_symbol(function, latent_symbol)
        },
    );

    assert!(
        function_ir_calls_symbol(entry_ir, latent_symbol)
            && !entry_ir.contains("switch i32 %step_tag"),
        "未调用的 higher-order effect 参数不应让外层 ordinary 直调用进入 Step dispatch:\n{entry_ir}"
    );
}

#[test]
pub(super) fn closure_call_without_outward_effect_stays_on_direct_call_surface() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun entry(): Int / (Ask) {
    val thunk: () -> Int / (Ask) = {
        handle {
            Ask.ask(41)
        } with {
            Ask.ask(seed) -> seed + 1
        }
    }
    return thunk()
}

fun main(): Int {
    return handle {
        entry()
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_matching(
        &ir,
        "ordinary entry calling closure without outward Step dispatch",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && !function.contains("switch i32 %step_tag")
                && (function_ir_calls_matching_symbol(
                    function,
                    stable_id_symbol_looks_like_closure_family,
                ) || function.contains("closure_dynamic_entry")
                    || function.contains("closure_env"))
        },
    );

    assert!(
        (function_ir_calls_matching_symbol(entry_ir, stable_id_symbol_looks_like_closure_family)
            || entry_ir.contains("closure_dynamic_entry")
            || entry_ir.contains("closure_env"))
            && !entry_ir.contains("switch i32 %step_tag"),
        "body 不 outward 的 closure 调用应保持 direct-call surface，而不是进入 Step dispatch:\n{entry_ir}"
    );
}

#[test]
pub(super) fn direct_call_with_real_outward_effect_uses_step_boundary_and_surface_resume_dispatch()
{
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun outward(): Int / (Ask) {
    Ask.ask(41)
}

fun entry(): Int / (Ask) {
    return outward()
}

fun main(): Int {
    return handle {
        entry()
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let outward_ir = function_ir_matching(&ir, "outward effect helper", |header, function| {
        !header.contains("@main(")
            && header.contains("lowered_direct_invoke")
            && !function.contains("switch i32 %step_tag")
            && function.contains("store i32 1")
            && function.contains("i64 41")
    });
    let entry_ir = function_ir_matching(&ir, "entry step-boundary helper", |header, function| {
        !header.contains("@main(")
            && header.contains("lowered_direct_invoke")
            && function.contains("switch i32 %step_tag")
            && function.contains("call %scoop.lowered.Step")
    });

    assert!(
        entry_ir.contains("call %scoop.lowered.Step") && entry_ir.contains("switch i32 %step_tag"),
        "ordinary direct outward-effect call 应改走 Step boundary，并按 step tag dispatch:\n{entry_ir}"
    );
    assert!(
        outward_ir.contains("store i32 1"),
        "effectful callee 自身应直接发布 outward Step case，而不是退回额外桥接 surface:\n{outward_ir}"
    );
    assert!(
        function_ir_count_matching(&ir, |header, _| {
            header.contains("surface_resume_owner_dispatch")
        }) >= 2,
        "direct outward path 应继续发布 entry/callee 的 authoritative surface-resume owner dispatch:\n{ir}"
    );
}

#[test]
pub(super) fn closure_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun entry(): Int / (Ask) {
    val thunk: () -> Int / (Ask) = {
        Ask.ask(41)
    }
    return thunk()
}

fun main(): Int {
    return handle {
        entry()
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let lambda_ir = function_ir_matching(
        &ir,
        "outward closure direct invoke helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_has_private_role(
                    llvm_function_symbol_name(function),
                    "lowered_direct_invoke",
                )
                && !function.contains("switch i32 %step_tag")
                && function.contains("store i32 1")
                && function.contains("i64 41")
        },
    );
    let entry_ir = function_ir_matching(
        &ir,
        "entry helper for outward closure call",
        |header, function| {
            !header.contains("@main(")
                && header.contains("lowered_direct_invoke")
                && function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        entry_ir.contains("switch i32 %step_tag"),
        "outward-effect closure call 应通过 Step boundary 做 step-tag dispatch:\n{entry_ir}"
    );
    let lambda_step_ty = llvm_function_return_named_struct_type(lambda_ir)
        .expect("expected hashed Step return type for outward closure body helper");
    assert!(
        stable_id_type_name_has_hashed_family(lambda_step_ty, "Step")
            && entry_ir.lines().any(|line| {
                line.contains(" call ")
                    && line.contains("@__scoop_priv0__lowered_direct_invoke__h")
                    && line.contains(&format!("%{lambda_step_ty}"))
            }),
        "当前默认路径会把单次 outward closure thunk 直接绑定到 authoritative lambda entry:\n{entry_ir}"
    );
}
