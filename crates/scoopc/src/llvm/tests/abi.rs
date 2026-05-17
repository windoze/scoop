//! ABI baseline tests + helpers: direct-extern-native-leaf, native-funptr-aggregate, managed-extern, virtual-call, etc.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

pub(super) fn assert_abi_baseline_direct_extern_native_leaf_contract() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

@Extern("scoop_test_gc_collect_in_native")
fun gcCollectInNative(): Unit

fun main() {
    val x: Any = f"hello {7}"
    @Unsafe do { gcCollectInNative() }
    val h: GcHandle = @Unsafe do { GC.handleNew(x) }
    @Unsafe do { GC.handleDrop(h) }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_matching(
        &ir,
        "direct extern native leaf helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_test_gc_collect_in_native")
        },
    );

    assert!(
        entry_ir.contains("call void @scoop_enter_native(")
            && entry_ir.contains("call void @scoop_test_gc_collect_in_native()")
            && entry_ir.contains("call void @scoop_leave_native()"),
        "direct `@Extern` native leaf 应保持 enter_native/native call/leave_native 三连序列:\n{entry_ir}"
    );
    assert!(
        entry_ir.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_0"),
        "direct `@Extern` native leaf 返回后应从 explicit frame home slot reload live root:\n{entry_ir}"
    );
    assert!(
        !ir.contains("@llvm.experimental.gc.statepoint"),
        "current direct `@Extern` baseline 应保持 plain native call，不重新进入 statepoint rewrite:\n{ir}"
    );

    let decl_line = llvm_declaration_line_matching(&ir, "extern native leaf declaration", |line| {
        line.contains("@scoop_test_gc_collect_in_native()")
    });
    let attr_group = llvm_attribute_group_for_declaration(&ir, decl_line)
        .expect("expected extern declaration to reference an LLVM attribute group");
    assert!(
        attr_group.contains("\"gc-leaf-function\""),
        "direct `@Extern` declaration 应继续标记 gc-leaf-function:\ndecl: {decl_line}\nattrs: {attr_group}"
    );
}

pub(super) fn assert_abi_baseline_native_funptr_aggregate_return_contract() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.unsafe.*

@Extern("scoop_test_get_make_int_pair_funptr")
fun get_make_int_pair_funptr(): FunPtr<(Int) -> (Int, Int)>

fun main(): Int {
    @Unsafe do {
        val fp: FunPtr<(Int) -> (Int, Int)> = get_make_int_pair_funptr()

        val a: (Int, Int) = fp(7)
        println(a._0)
        println(a._1)

        val b: (Int, Int) = fp.invoke(9)
        println(b._0)
        println(b._1)
    }
    0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_matching(
        &ir,
        "native funptr aggregate-return helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_test_get_make_int_pair_funptr")
                && function.contains("call { i64, i64 } %")
        },
    );
    let leave_idx = entry_ir
        .find("call void @scoop_leave_native()")
        .expect("expected direct extern getter to leave native before indirect funptr calls");
    let indirect_window = &entry_ir[leave_idx..];
    let native_funptr_calls = indirect_window.matches("call { i64, i64 } %").count();

    assert!(
        indirect_window.contains("inttoptr i64")
            && native_funptr_calls >= 2
            && indirect_window.contains("extractvalue { i64, i64 }"),
        "native `FunPtr<(Int) -> (Int, Int)>` 应继续按目标 ABI 直接返回 aggregate，而不是回 ordinary hidden sret:\n{indirect_window}"
    );
    assert!(
        !indirect_window.contains("funptr_call_sret") && !indirect_window.contains(" sret("),
        "native `FunPtr` aggregate return 不应重新落回 hidden sret 路径:\n{indirect_window}"
    );
}

pub(super) fn assert_native_funptr_indirect_call_preserves_native_boundary_contract() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.unsafe.*

@Extern("scoop_test_get_gc_collect_in_native_funptr")
fun get_gc_collect_in_native_funptr(): FunPtr<() -> Unit>

fun main() {
    val x: Any = f"hello {7}"

    @Unsafe do {
        val fp: FunPtr<() -> Unit> = get_gc_collect_in_native_funptr()
        fp()
    }

    val h: GcHandle = @Unsafe do { GC.handleNew(x) }
    @Unsafe do { GC.handleDrop(h) }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir =
        function_ir_matching(&ir, "native funptr boundary helper", |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_test_get_gc_collect_in_native_funptr")
                && function.contains("call void %")
        });

    let getter_idx = entry_ir
        .find("call i64 @scoop_test_get_gc_collect_in_native_funptr()")
        .expect("expected direct extern getter call");
    let post_getter = &entry_ir[getter_idx..];
    let indirect_enter_idx = post_getter
        .find("call void @scoop_enter_native(")
        .expect("expected indirect funptr call to re-enter native boundary");
    let indirect_window = &post_getter[indirect_enter_idx..];

    assert!(
        entry_ir.matches("call void @scoop_enter_native(").count() >= 2
            && entry_ir.matches("call void @scoop_leave_native()").count() >= 2,
        "native `FunPtr` indirect call 应与 getter 一样受 native boundary 包裹:\n{entry_ir}"
    );
    assert!(
        entry_ir.contains("inttoptr i64")
            && indirect_window.contains("call void %")
            && indirect_window.contains("call void @scoop_leave_native()"),
        "native `FunPtr` 间接调用应生成 enter_native/indirect call/leave_native 三连序列:\n{indirect_window}"
    );
    assert!(
        entry_ir.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_0"),
        "native `FunPtr` 返回后应从 explicit frame home slot reload live root:\n{entry_ir}"
    );
    assert!(
        !ir.contains("@llvm.experimental.gc.statepoint"),
        "native `FunPtr` boundary 仍应保持 plain native call，不重新进入 statepoint rewrite:\n{ir}"
    );
}

pub(super) fn assert_native_callable_aggregate_direct_indirect_parity_contract() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.unsafe.*

@Extern("scoop_test_make_int_pair")
fun make_int_pair(seed: Int): (Int, Int)

@Extern("scoop_test_get_make_int_pair_funptr")
fun get_make_int_pair_funptr(): FunPtr<(Int) -> (Int, Int)>

fun main(): Int {
    @Unsafe do {
        val direct: (Int, Int) = make_int_pair(7)
        println(direct._0)
        println(direct._1)

        val fp: FunPtr<(Int) -> (Int, Int)> = get_make_int_pair_funptr()
        val indirect: (Int, Int) = fp(7)
        println(indirect._0)
        println(indirect._1)

        val invoked: (Int, Int) = fp.invoke(9)
        println(invoked._0)
        println(invoked._1)
    }
    0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_matching(
        &ir,
        "native aggregate direct/indirect parity helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_test_make_int_pair")
                && function.contains("@scoop_test_get_make_int_pair_funptr")
        },
    );
    let getter_call_idx = entry_ir
        .find("call i64 @scoop_test_get_make_int_pair_funptr()")
        .expect("expected direct extern getter call for aggregate funptr parity");
    let indirect_window = &entry_ir[getter_call_idx..];
    let native_funptr_calls = indirect_window.matches("call { i64, i64 } %").count();

    assert!(
        entry_ir.contains("call { i64, i64 } @scoop_test_make_int_pair(i64 7)")
            && indirect_window.contains("inttoptr i64")
            && native_funptr_calls >= 2,
        "direct `@Extern` 与 native `FunPtr` 应共享同一 target aggregate-return ABI:\n{entry_ir}"
    );
    assert!(
        entry_ir.matches("call void @scoop_enter_native(").count() >= 4
            && entry_ir.matches("call void @scoop_leave_native()").count() >= 4,
        "direct/indirect native aggregate 调用都应通过同一 native boundary scaffold:\n{entry_ir}"
    );
    assert!(
        !entry_ir.contains("funptr_call_sret") && !entry_ir.contains(" sret("),
        "native callable aggregate-return parity 不应重新落回 hidden sret 路径:\n{entry_ir}"
    );
}

pub(super) fn assert_abi_baseline_sysroot_string_helper_contract() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val word: String = "hello".substring(1, 4)
    return if (word == "ell") 1 else 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let substring_ir = function_ir_matching(
        &ir,
        "compiled sysroot substring helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_symbol_mentions_fqn(
                    llvm_function_symbol_name(function),
                    "scoop.lang.string.substring",
                )
        },
    );

    assert!(
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(substring_ir),
            "scoop.lang.string.substring"
        ),
        "single-file LLVM 路径应把可编译 sysroot 源中的 substring helper 编进当前模块"
    );
}

pub(super) fn assert_sysroot_scalar_string_bridge_contract() {
    let source = SourceFile::new_virtual(
        "<mem>/scalar_string_bridge_contract.scoop",
        r#"
package fixtures.scalar_string_bridge

import scoop.core.*

fun main(): Int {
    val narrow: Float32 = 1.5
    println(scoopAbiIntToString(42))
    println(scoopAbiCharToString('A'))
    println(scoopAbiFloat32ToString(narrow))
    println(scoopAbiFloat64ToString(2.25))
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let int_bridge_ir = function_ir_matching(
        &ir,
        "compiled sysroot Int scalar String bridge helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_symbol_mentions_fqn(
                    llvm_function_symbol_name(function),
                    "scoop.core.scoopAbiIntToString",
                )
        },
    );
    assert!(
        int_bridge_ir.contains("@scoop_int_to_string(")
            && !int_bridge_ir.contains("@scoop_enter_native")
            && !int_bridge_ir.contains("@scoop_leave_native"),
        "compiled sysroot Int bridge helper 应通过 audited runtime symbol 返回 String，且不复用 native boundary scaffold:\n{int_bridge_ir}"
    );

    let char_bridge_ir = function_ir_matching(
        &ir,
        "compiled sysroot Char scalar String bridge helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_symbol_mentions_fqn(
                    llvm_function_symbol_name(function),
                    "scoop.core.scoopAbiCharToString",
                )
        },
    );
    assert!(
        char_bridge_ir.contains("@scoop_char_to_string("),
        "compiled sysroot Char bridge helper 应调用 scoop_char_to_string:\n{char_bridge_ir}"
    );

    let float32_bridge_ir = function_ir_matching(
        &ir,
        "compiled sysroot Float32 scalar String bridge helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_symbol_mentions_fqn(
                    llvm_function_symbol_name(function),
                    "scoop.core.scoopAbiFloat32ToString",
                )
        },
    );
    assert!(
        float32_bridge_ir.contains("@scoop_float32_to_string("),
        "compiled sysroot Float32 bridge helper 应调用 scoop_float32_to_string:\n{float32_bridge_ir}"
    );

    let float64_bridge_ir = function_ir_matching(
        &ir,
        "compiled sysroot Float64 scalar String bridge helper",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_symbol_mentions_fqn(
                    llvm_function_symbol_name(function),
                    "scoop.core.scoopAbiFloat64ToString",
                )
        },
    );
    assert!(
        float64_bridge_ir.contains("@scoop_float64_to_string("),
        "compiled sysroot Float64 bridge helper 应调用 scoop_float64_to_string:\n{float64_bridge_ir}"
    );

    assert!(
        maybe_function_ir_matching(&ir, |_, function| {
            stable_id_symbol_mentions_fqn(
                llvm_function_symbol_name(function),
                "scoop.core.__scoop_runtime_int_to_string_bridge",
            )
        })
        .is_none(),
        "declaration-only runtime bridge intrinsic 不应物化成 ordinary helper 符号:\n{ir}"
    );
}

pub(super) fn assert_managed_extern_direct_call_uses_ordinary_managed_contract() {
    let source = SourceFile::new_virtual(
        "<mem>/managed_extern_string_return.scoop",
        r#"
package fixtures.managed_abi

import scoop.core.*

@Extern("managed_string_helper", abi = "scoop")
fun managedStringHelper(): String

fun main() {
    val keep: String = f"keep-{41}"
    val message: String = managedStringHelper()
    println(keep)
    println(message)
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let call_ir = function_ir_matching(
        &ir,
        "managed extern ordinary direct call helper",
        |_, function| function.contains("call ptr addrspace(1) @managed_string_helper()"),
    );

    assert!(
        call_ir.contains("call ptr addrspace(1) @managed_string_helper()"),
        "`ExternAbi::Scoop` 直接调用应走 ordinary managed return 路径:\n{call_ir}"
    );
    assert!(
        !call_ir.contains("@scoop_enter_native") && !call_ir.contains("@scoop_leave_native"),
        "`ExternAbi::Scoop` 不得复用 native boundary scaffold:\n{call_ir}"
    );

    let decl_line = llvm_declaration_line_matching(&ir, "managed extern declaration", |line| {
        line.contains("@managed_string_helper()")
    });
    assert!(
        decl_line.starts_with("declare ptr addrspace(1) @managed_string_helper()"),
        "`ExternAbi::Scoop` declaration 应使用 ordinary managed String surface，而不是 native leaf declaration:\n{decl_line}\n{ir}"
    );
    if let Some(attr_group) = llvm_attribute_group_for_declaration(&ir, decl_line) {
        assert!(
            !attr_group.contains("\"gc-leaf-function\""),
            "managed extern declaration 不应带 gc-leaf-function:\ndecl: {decl_line}\nattrs: {attr_group}"
        );
    }
}

pub(super) fn assert_managed_extern_aggregate_return_uses_hidden_sret_contract() {
    let source = SourceFile::new_virtual(
        "<mem>/managed_extern_aggregate_return.scoop",
        r#"
package fixtures.managed_abi

import scoop.core.*

@Extern("managed_make_pair", abi = "scoop")
fun managedMakePair(seed: Int): (Int, Int)

fun main(): Int {
    val pair: (Int, Int) = managedMakePair(7)
    println(pair._0)
    println(pair._1)
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let call_ir = function_ir_matching(
        &ir,
        "managed extern hidden-sret aggregate helper",
        |_, function| {
            function.contains("call_sret = alloca")
                && function.contains("@managed_make_pair")
                && function.contains("call void @managed_make_pair(")
        },
    );

    assert!(
        call_ir.contains("call_sret = alloca")
            && call_ir.contains("call void @managed_make_pair(")
            && call_ir.contains(" sret("),
        "managed extern aggregate return 应走 ordinary hidden sret，而不是 native aggregate return:\n{call_ir}"
    );
    assert!(
        !call_ir.contains("call { i64, i64 } @managed_make_pair"),
        "managed extern aggregate return 不得复用 native direct aggregate result ABI:\n{call_ir}"
    );

    let decl_line =
        llvm_declaration_line_matching(&ir, "managed extern hidden-sret declaration", |line| {
            line.contains("@managed_make_pair(")
        });
    assert!(
        decl_line.starts_with("declare void @managed_make_pair(") && decl_line.contains(" sret("),
        "managed extern aggregate declaration 应发布 hidden sret surface:\n{decl_line}\n{ir}"
    );
    if let Some(attr_group) = llvm_attribute_group_for_declaration(&ir, decl_line) {
        assert!(
            !attr_group.contains("\"gc-leaf-function\""),
            "managed extern aggregate declaration 不应带 gc-leaf-function:\ndecl: {decl_line}\nattrs: {attr_group}"
        );
    }
}

// P0-T01 ABI baseline audit：集中冻结 current two-surface callable contract + compiled string helper。
#[test]
pub(super) fn abi_baseline_direct_extern_native_leaf_preserves_enter_leave_native_sequence() {
    assert_abi_baseline_direct_extern_native_leaf_contract();
}

#[test]
pub(super) fn abi_baseline_native_funptr_aggregate_return_uses_native_result_abi() {
    assert_abi_baseline_native_funptr_aggregate_return_contract();
}

#[test]
pub(super) fn native_callable_funptr_indirect_call_uses_enter_leave_native_boundary() {
    assert_native_funptr_indirect_call_preserves_native_boundary_contract();
}

#[test]
pub(super) fn native_callable_direct_and_indirect_aggregate_return_share_target_abi() {
    assert_native_callable_aggregate_direct_indirect_parity_contract();
}

#[test]
pub(super) fn managed_extern_direct_call_uses_ordinary_managed_contract() {
    assert_managed_extern_direct_call_uses_ordinary_managed_contract();
}

#[test]
pub(super) fn managed_extern_aggregate_return_uses_hidden_sret_contract() {
    assert_managed_extern_aggregate_return_uses_hidden_sret_contract();
}

#[test]
pub(super) fn virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

open class Base() {
    open fun ping(): Int / (Ask) {
        Ask.ask(1)
    }
}

class Derived() : Base() {
    override fun ping(): Int / (Ask) {
        Ask.ask(41)
    }
}

fun helper(base: Base): Int / (Ask) {
    return base.ping()
}

fun main(): Int {
    return handle {
        helper(Derived())
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "virtual dispatch outward helper",
        |header, function| {
            !header.contains("@main(")
                && function.contains("load_vtable_fn")
                && function.contains("call %scoop.lowered.Step")
                && function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        helper_ir.contains("load_vtable_fn")
            && helper_ir.contains("call %scoop.lowered.Step")
            && helper_ir.contains("switch i32 %step_tag"),
        "默认 virtual-cone path 的 outward vtable helper 应走 Step dispatch，而不是缺失 helper body 或回落旧 wrapper:\n{helper_ir}"
    );
    assert!(
        ir.contains("surface_resume_owner_dispatch"),
        "默认 virtual-cone path 的 outward vtable helper 应继续发布 authoritative surface-resume owner dispatch:\n{ir}"
    );
}

#[test]
pub(super) fn interface_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

interface IFace {
    fun ping(): Int / (Ask)
}

class Impl() : IFace {
    fun ping(): Int / (Ask) {
        Ask.ask(52)
    }
}

fun helper(face: IFace): Int / (Ask) {
    return face.ping()
}

fun main(): Int {
    return handle {
        helper(Impl())
    } with {
        Ask.ask(seed) -> seed
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir =
        function_ir_matching(&ir, "itable dispatch outward helper", |header, function| {
            !header.contains("@main(")
                && function.contains("itable_lookup")
                && function.contains("load_itable_fn")
                && function.contains("call %scoop.lowered.Step")
        });

    assert!(
        helper_ir.contains("itable_lookup")
            && helper_ir.contains("load_itable_fn")
            && helper_ir.contains("call %scoop.lowered.Step"),
        "默认 virtual-cone path 的 outward itable helper 应走 Step dispatch，而不是缺失 helper body 或回落旧 wrapper:\n{helper_ir}"
    );
    assert!(
        ir.contains("surface_resume_owner_dispatch"),
        "默认 virtual-cone path 的 outward itable helper 应继续发布 authoritative surface-resume owner dispatch:\n{ir}"
    );
}

#[test]
pub(super) fn object_value_init_access_stays_plain_without_effect_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

object BoomObject {
    init {
        ping()
    }

    val marker: Int = 1
}

fun ping() {}

fun helper(): Int {
    val _obj = BoomObject
    return 7
}

fun main(): Int {
    return helper()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "user helper reading object value init without effect boundary",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_ir_contains_hidden_init_call(function)
                && !function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        stable_id_ir_contains_hidden_init_call(helper_ir)
            && !helper_ir.contains("switch i32 %step_tag"),
        "object value init access 应保持 plain once-init call surface，而不是进入 Step dispatch:\n{helper_ir}"
    );
}

#[test]
pub(super) fn object_property_init_access_stays_plain_without_effect_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

object Holder {
    val broken: Int = 7
}

fun helper(): Int {
    return Holder.broken
}

fun main(): Int {
    return helper()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "user helper reading object property init without effect boundary",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_ir_contains_hidden_init_call(function)
                && !function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        stable_id_ir_contains_hidden_init_call(helper_ir)
            && !helper_ir.contains("switch i32 %step_tag"),
        "object property init access 应保持 plain once-init call surface，而不是进入 Step dispatch:\n{helper_ir}"
    );
}

#[test]
pub(super) fn top_level_immutable_init_access_stays_plain_without_effect_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

val Broken: Int = 7

fun helper(): Int {
    return Broken
}

fun main(): Int {
    return helper()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "user helper reading top-level immutable init without effect boundary",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && stable_id_ir_contains_hidden_init_call(function)
                && !function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        stable_id_ir_contains_hidden_init_call(helper_ir)
            && !helper_ir.contains("switch i32 %step_tag"),
        "top-level immutable init access 应保持 plain once-init call surface，而不是进入 Step dispatch:\n{helper_ir}"
    );
}

#[test]
pub(super) fn pure_extern_call_does_not_install_effect_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

@Extern("scoop_test_add_int")
fun nativeAdd(a: Int, b: Int): Int

fun helper(): Int {
    return @Unsafe do { nativeAdd(1, 2) }
}

fun main(): Int {
    return helper()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "user helper issuing plain extern call without effect boundary",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_test_add_int")
                && !function.contains("switch i32 %step_tag")
        },
    );

    assert!(
        helper_ir.contains("@scoop_test_add_int") && !helper_ir.contains("switch i32 %step_tag"),
        "ordinary `@Extern` 调用应保持 plain native call surface，而不是进入 Step dispatch:\n{helper_ir}"
    );
}

#[test]
pub(super) fn class_ctor_uses_concrete_generic_instance_layout() {
    let source = SourceFile::new_virtual(
        "<mem>/generic_class_instance_layout.scoop",
        r#"
package a

import scoop.core.*

class Box<T>(var value: T)

fun main(): Int {
    val box: Box<String> = Box("hi")
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let ir = module.print_to_string().to_string();

    let payload_defs = ir
        .lines()
        .filter(|line| {
            line.starts_with("%scoop.lowered.ClassPayload__h") && line.contains(" = type ")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        payload_defs.len(),
        1,
        "Box<String> constructor should publish exactly one concrete class payload type\n{ir}"
    );
    let payload_ty_name = payload_defs[0]
        .split_once(" = ")
        .map(|(name, _)| name.trim_start_matches('%'))
        .expect("class payload type definition should contain name");
    assert!(
        payload_defs[0].contains("type { ptr addrspace(1) }"),
        "Box<String>.value should lower as the concrete String GC pointer field, not generic T\n{}",
        payload_defs[0]
    );
    let object_defs = ir
        .lines()
        .filter(|line| {
            line.starts_with("%scoop.lowered.ClassObject__h") && line.contains(" = type ")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        object_defs.len(),
        1,
        "Box<String> constructor should publish exactly one concrete class object type\n{ir}"
    );
    let object_ty_name = object_defs[0]
        .split_once(" = ")
        .map(|(name, _)| name.trim_start_matches('%'))
        .expect("class object type definition should contain name");
    assert!(
        object_defs[0].contains(&format!("%{payload_ty_name}")),
        "class object type 应内嵌 concrete payload type，而不是 generic/raw layout\n{}",
        object_defs[0]
    );
    assert!(
        ir.contains("@scoop_alloc_typed")
            && ir.contains(&format!(
                "%class_payload_gep = getelementptr inbounds nuw %{object_ty_name}"
            ))
            && ir.contains(&format!(
                "%class_field_gep = getelementptr inbounds nuw %{payload_ty_name}"
            ))
            && ir.contains("@__scoop_priv0__class_type_desc__h"),
        "constructor allocation 应通过 typed descriptor 局部值发布 concrete Box<String> 分配路径，而不是锁死 descriptor symbol 文本\n{ir}"
    );
}

#[test]
pub(super) fn generic_class_init_raise_cleanup_uses_stable_type_driven_box_naming() {
    let source = SourceFile::new_virtual(
        "<mem>/generic_class_init_raise_cleanup.scoop",
        r#"
package a

import scoop.core.*

class Box<B>(val value: B) {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }
}

fun main(): Int {
    try {
        val _x: Box<Int> = Box(1)
    } catch (e: RuntimeError) {
        // ignore
    }
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source)
        .expect("generic class init cleanup path 应可为 type-driven private box 命名生成 IR");

    assert!(
        ir.contains("scoop.lowered.MirValueBox__h")
            || ir.contains("scoop.lowered.EffectTransportBox__h"),
        "generic class init cleanup path 应继续发布 stable-hash private box family，而不是因为 type param resolver 缺失而失败\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_priv0__mir_value_box_type_desc__h")
            || ir.contains("@__scoop_priv0__lowered_effect_transport_box__h"),
        "generic class init cleanup path 应继续发布对应的 stable private descriptor/global anchor\n{ir}"
    );
}

#[test]
pub(super) fn indirect_gc_aggregate_param_syncs_explicit_frame_home_slot_on_entry() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

struct Named(val name: String, val score: Int)

fun keep(named: Named): String {
    __scoop_gc_collect()
    return named.name
}

fun main() {
    println(keep(Named { name: "hi", score: 1 }))
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let keep_ir = function_ir_matching(
        &ir,
        "managed aggregate-parameter helper with safepoint",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_gc_collect_safepoint")
                && function.contains("explicit_root_frame_slot_0")
        },
    );

    let stores_into_home_slot = keep_ir
        .lines()
        .filter(|line| {
            line.contains("store ptr addrspace(1)")
                && line.contains(", ptr %explicit_root_frame_slot_0")
                && !line.contains(" null,")
        })
        .count();

    assert!(
        stores_into_home_slot >= 1,
        "expected indirect GC aggregate param to sync its ref leaf into explicit frame home slot before safepoint\n{keep_ir}"
    );
}

#[test]
pub(super) fn single_file_minimal_ir_includes_compilable_sysroot_string_helpers() {
    assert_abi_baseline_sysroot_string_helper_contract();
}

#[test]
pub(super) fn abi_baseline_compiled_sysroot_string_helper_stays_in_module() {
    assert_abi_baseline_sysroot_string_helper_contract();
}

#[test]
pub(super) fn compiled_sysroot_scalar_string_bridge_helpers_stay_in_module() {
    assert_sysroot_scalar_string_bridge_contract();
}

#[test]
pub(super) fn box_int_to_any_uses_addrspace_1_ref_pointer() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    __scoop_gc_collect()
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("addrspace(1)"),
        "IR 应包含 addrspace(1)（GC-managed 引用指针）"
    );
    assert!(
        ir.contains("@scoop_alloc_typed"),
        "装箱到 Any 应调用/声明 scoop_alloc_typed"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "当前阶段的装箱路径不应依赖 addrspacecast 回退到 addrspace(0)"
    );
}

#[test]
pub(super) fn sync_mutex_runtime_calls_use_addrspace_1_object_pointers() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.sync.*

fun main(): Int {
    val m: Mutex = mutexCreate()
    m.lock()
    m.unlock()
    m.destroy()
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_sync_mutex_create"),
        "IR 应包含对 scoop_sync_mutex_create 的引用"
    );
    assert!(
        ir.contains("addrspace(1)"),
        "IR 应包含 addrspace(1)（GC-managed 引用指针）"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "sync 相关调用不应依赖 addrspacecast 回退到 addrspace(0)"
    );
}

#[test]
pub(super) fn string_literal_uses_addrspace_1_gc_string_object() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val s: String = "hi"
    println(s)
    __scoop_gc_collect()
    println(s)
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_println"),
        "IR 应包含对 scoop_println 的引用"
    );
    assert!(
        ir.contains("addrspace(1)"),
        "String 应为 addrspace(1) GC-managed 指针"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "String 相关调用不应依赖 addrspacecast 回退到 addrspace(0)"
    );
}

#[test]
pub(super) fn object_member_call_uses_gc_managed_singleton_receiver() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

object Helper {
    fun run(): Int {
        return 7
    }
}

fun main(): Int {
    return Helper.run()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    let (_receiver_local, singleton_slot_symbol) = ir
        .lines()
        .filter_map(llvm_load_gc_ref_from_global)
        .find(|(_, symbol)| {
            llvm_ir_global_definition(&ir, symbol)
                .is_some_and(|line| line.contains("internal global ptr addrspace(1) null"))
        })
        .expect(
            "object member call should reload the singleton receiver from a GC-managed slot global",
        );
    assert!(
        llvm_ir_global_definition(&ir, singleton_slot_symbol)
            .is_some_and(|line| line.contains("internal global ptr addrspace(1) null")),
        "object 单例槽应保存 GC-managed receiver 指针，而不是把某个固定全局名当金标准\n{ir}"
    );
    assert!(
        stable_id_symbol_has_private_role(singleton_slot_symbol, "object_instance"),
        "object singleton slot 应使用 object_instance private role，实际符号: {singleton_slot_symbol}"
    );
    assert!(
        ir.contains("@scoop_alloc_typed"),
        "object 单例值应通过 typed alloc 生成真实 Ref 对象"
    );
    assert!(
        ir.contains("@__scoop_priv0__object_type_desc__h"),
        "object singleton runtime metadata/type names 应改走 stable private naming，而不是 sanitize/object FQN 拼接\n{ir}"
    );
    let run_ir = function_ir_matching(&ir, "object member method body", |header, function| {
        !header.contains("@main(")
            && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
            && header.contains("ptr addrspace(1)")
            && !function.contains(" call ")
    });
    let run_symbol = llvm_function_symbol_name(run_ir);
    assert!(
        ir.lines().any(|line| {
            line.contains(" call i64 ")
                && llvm_line_mentions_symbol(line, run_symbol)
                && line.contains("ptr addrspace(1)")
        }),
        "object member call 应把 addrspace(1) receiver 传给成员函数"
    );
    assert!(
        !ir.lines().any(|line| {
            line.contains(" call i64 ")
                && llvm_line_mentions_symbol(line, run_symbol)
                && (line.contains(&format!("ptr @{singleton_slot_symbol}"))
                    || line.contains(&format!("ptr @\"{singleton_slot_symbol}\"")))
        }),
        "member call 不应再把默认地址空间全局地址直接当 receiver 传递"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "object member call 修复不应退回 addrspacecast 打补丁"
    );
}

#[test]
pub(super) fn println_int_lowers_via_string_formatting_without_print_int_helpers() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    println(123)
    __scoop_gc_collect()
    println(-42)
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_println"),
        "IR 应包含对 scoop_println 的引用（与 String 路径对齐）"
    );
    assert!(
        ir.contains("@scoop_int_to_string"),
        "IR 应通过 scoop_int_to_string 走统一 Int->String runtime 路径"
    );
    assert!(
        !ir.contains("@scoop_format_i64"),
        "println(Int) 不应再回旧的格式化 helper 名称 `scoop_format_i64`"
    );
    assert!(
        !ir.contains("@scoop_println_i64"),
        "println(Int) 不应再依赖 runtime 的 scoop_println_i64 绕路"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "println(Int)->String 的路径不应依赖 addrspacecast"
    );
}

#[test]
pub(super) fn array_of_any_uses_ir_direct_ref_load_without_ptr_to_u64() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    val b: Any = 2
    val xs: Array<Any> = [a, b]
    val v: Any = xs.get(0)
    __scoop_gc_collect()
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_mutable_array_push_ref") && ir.contains("@scoop_mutable_array_freeze"),
        "Array<Any> 的 array literal 应走 MutableArray.push_ref + freeze 路径"
    );
    assert!(
        ir.contains("array_len_gep")
            && ir.contains("array_data_offset_gep")
            && ir.contains("array_get_load = load ptr addrspace(1)"),
        "Array<Any>.get 应直接 GEP/load `ScoopArray` layout，而不是回 runtime helper:\n{ir}"
    );
    assert!(
        !ir.contains("@scoop_array_get_ref"),
        "Array<Any>.get 不应再声明/调用 scoop_array_get_ref:\n{ir}"
    );
    assert!(
        !ir.contains("ptr_to_u64"),
        "ref 元素路径不应把 GC 指针编码为 u64（ptr_to_u64）"
    );
    assert!(
        !ir.contains("u64_to_ref"),
        "ref 元素路径不应从 u64 解码回 GC 指针（u64_to_ref）"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "ref array 路径不应引入 addrspacecast"
    );
}

#[test]
pub(super) fn array_of_string_uses_ir_direct_ref_load_and_write_barrier_without_ptr_to_u64() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<String> = ["a", "b"]
    xs.set(0, "z")
    val v: String = xs.get(0)
    println(v)
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_mutable_array_push_ref"),
        "MutableArray<String> 的 array literal 应走 MutableArray.push_ref 路径"
    );
    assert!(
        ir.contains("mutable_array_len_gep")
            && ir.contains("mutable_array_data_gep")
            && ir.contains("array_get_load = load ptr addrspace(1)"),
        "MutableArray<String>.get 应经 out-of-line data 指针 load，而不是回 runtime helper 或 inline ScoopArray layout:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_gc_write_barrier") && ir.contains("gc_promotion_barrier"),
        "MutableArray<String>.set 应直接写 out-of-line slot 并经 promotion write barrier，而不是回 runtime helper:\n{ir}"
    );
    assert!(
        !ir.contains("@scoop_array_get_ref") && !ir.contains("@scoop_array_set_ref"),
        "Array<String>.get / MutableArray<String>.set 不应再声明旧 ref helper:\n{ir}"
    );
    assert!(
        !ir.contains("ptr_to_u64"),
        "String 元素路径不应把 GC 指针编码为 u64（ptr_to_u64）"
    );
    assert!(
        !ir.contains("u64_to_string"),
        "String 元素路径不应从 u64 解码回 GC 字符串指针（u64_to_string）"
    );
}

#[test]
pub(super) fn mutable_array_size_loads_len_field() {
    let source = SourceFile::new_virtual(
        "<mem>/mutable_array_size_layout.scoop",
        r#"
package fixtures.p3t02.size

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<Int> = [1, 2, 3]
    return xs.size()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let main_ir = function_ir_matching(&ir, "mutable array size main", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.p3t02.size.main",
        )
    });

    assert!(
        main_ir.contains("mutable_array_len_gep")
            && main_ir.contains("mutable_array_len = load i64"),
        "MutableArray.size 应从 ScoopMutableArray.len 字段读取:\n{main_ir}"
    );
    assert!(
        !main_ir.contains("array_data_offset_gep"),
        "MutableArray.size 不应触碰 inline ScoopArray data_offset 字段:\n{main_ir}"
    );
}

#[test]
pub(super) fn mutable_array_get_indirect_through_data_ptr() {
    let source = SourceFile::new_virtual(
        "<mem>/mutable_array_get_layout.scoop",
        r#"
package fixtures.p3t02.get

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<Int> = [11, 22]
    return xs.get(1)
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let main_ir = function_ir_matching(&ir, "mutable array get main", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.p3t02.get.main",
        )
    });

    assert!(
        main_ir.contains("mutable_array_data_gep")
            && main_ir.contains("mutable_array_data = load ptr")
            && main_ir.contains("array_get_load = load i64"),
        "MutableArray.get 应先 load out-of-line data 指针再按元素 stride load:\n{main_ir}"
    );
    assert!(
        !main_ir.contains("array_data_offset_gep"),
        "MutableArray.get 不应沿用 inline ScoopArray trailing-data 路径:\n{main_ir}"
    );
}

#[test]
pub(super) fn mutable_array_set_emits_write_barrier_for_ref_element() {
    let source = SourceFile::new_virtual(
        "<mem>/mutable_array_set_ref_layout.scoop",
        r#"
package fixtures.p3t02.setref

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<String> = ["a", "b"]
    xs.set(1, "z")
    return xs.size()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let main_ir = function_ir_matching(&ir, "mutable array set ref main", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.p3t02.setref.main",
        )
    });

    assert!(
        main_ir.contains("mutable_array_data_gep")
            && main_ir.contains("store ptr addrspace(1)")
            && main_ir.contains("gc_promotion_barrier")
            && main_ir.contains("@scoop_gc_write_barrier"),
        "MutableArray<String>.set 应写 out-of-line ref slot 后调用 NULL-slot promotion barrier:\n{main_ir}"
    );
}

#[test]
pub(super) fn array_size_still_inline_after_dispatch_split() {
    let source = SourceFile::new_virtual(
        "<mem>/array_size_inline_layout.scoop",
        r#"
package fixtures.p3t02.inlinearr

import scoop.core.*

fun main(): Int {
    val xs: Array<Int> = [7, 9]
    return xs.size()
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let main_ir = function_ir_matching(&ir, "array size inline main", |_, function| {
        stable_id_symbol_mentions_fqn(
            llvm_function_symbol_name(function),
            "fixtures.p3t02.inlinearr.main",
        )
    });

    assert!(
        main_ir.contains("array_len_gep") && main_ir.contains("array_len = load i64"),
        "Array.size 应保持 inline ScoopArray.len 字段读取:\n{main_ir}"
    );
    assert!(
        !main_ir.contains("mutable_array_len_gep") && !main_ir.contains("mutable_array_data_gep"),
        "Array.size 不应漂移到 MutableArray out-of-line layout:\n{main_ir}"
    );
}

#[test]
pub(super) fn array_redundant_get_o2_cses_direct_load() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/p4_t01e_array_cse_o2.scoop",
        r#"
package fixtures.p4t01e

import scoop.core.*

fun main(): Int {
    val xs: Array<Int> = [11, 22]
    val a: Int = xs.get(0)
    val b: Int = xs.get(0)
    return a + b
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module_with_opt_level(&session, &source, &context, OptLevel::O2)
        .unwrap()
        .print_to_string()
        .to_string();
    let main_ir = function_ir_matching(&ir, "P4-T01e O2 array CSE main", |_, function| {
        stable_id_symbol_mentions_fqn(llvm_function_symbol_name(function), "fixtures.p4t01e.main")
    });

    assert!(
        !main_ir.contains("@scoop_array_get_u64")
            && !main_ir.contains("@__scoop_abi0_fun__scoop_core_Array_get"),
        "redundant get O2 path 不应退回 helper/method call:\n{main_ir}"
    );
    assert_eq!(
        main_ir.matches("array_get_load = load i64").count(),
        1,
        "redundant get O2 path 应把两次 xs.get(0) CSE 成单次 direct load:\n{main_ir}"
    );
}

#[test]
pub(super) fn enum_single_field_non_scalar_payload_uses_boxed_variant_path() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Result {
    Ok(val point: Point),
    Msg(val payload: (String, Int)),
    Err(val code: Int),
}

fun main(): Int {
    val ok: Result = Ok(Point { x: 7, y: 8 })
    val msg: Result = Msg(("hello", 30))
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.lines().any(|line| {
            line.starts_with("%scoop.lowered.EnumBoxedPayloadObject__h")
                && line.contains(" = type { %scoop.runtime.ScoopGcObjectHeader, %scoop.lowered.EnumBoxedPayloadFields__h")
        }),
        "single-field non-scalar payload 应生成 hashed boxed payload object type\n{ir}"
    );
    assert!(
        ir.lines()
            .filter(|line| {
                line.starts_with("@__scoop_priv0__enum_boxed_payload_type_desc__h")
                    && line.contains("%scoop.runtime.ScoopTypeDescriptor")
            })
            .count()
            >= 2,
        "single-field struct/tuple payload 应生成 hashed boxed payload type descriptor global\n{ir}"
    );
    assert!(
        ir.matches("rt_alloc_enum_boxed_payload").count() >= 2
            && ir.matches("enum_boxed_payload_gep").count() >= 2,
        "boxed non-scalar enum variant 应通过 descriptor-backed typed alloc/materialize path，而不是锁死具体 type-desc symbol 名字\n{ir}"
    );
}

#[test]
pub(super) fn missing_main_is_reported() {
    let source = SourceFile::new_virtual("<mem>", "package a\nfun not_main() {}");
    let session = Session::new().unwrap();
    let err = emit_minimal_main_ir(&session, &source).unwrap_err();

    assert!(
        matches!(err, LlvmEmitError::MissingEntryMain)
            || err.to_string().contains("找不到入口函数 `fun main`"),
        "missing main 应保持稳定错误，而不是静默继续：{err}"
    );
}

#[test]
pub(super) fn minimal_main_obj_written_is_non_empty() {
    let dir = make_temp_dir("minimal_main_obj_written_is_non_empty");
    let output = dir.join("main.o");

    let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
    let session = Session::new().unwrap();
    emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

    let size = std::fs::metadata(&output).unwrap().len();
    assert!(size > 0, "object 文件不应为空");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(super) fn minimal_main_obj_omits_stackmap_section_by_default() {
    let dir = make_temp_dir("minimal_main_obj_omits_stackmap_section");
    let output = dir.join("main.o");

    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main() {
    // 强制触发 `Int -> Any` 装箱（heap alloc），让 statepoint pipeline 产出 stackmap records。
    val a: Any = 1
}
"#,
    );
    let session = Session::new().unwrap();
    emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

    let bytes = std::fs::read(&output).unwrap();
    let obj = object::File::parse(&*bytes).expect("failed to parse object file");

    assert!(
        !object_contains_stackmap_section(&obj),
        "default explicit mode should not emit a stackmap section"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(super) fn minimal_main_obj_with_live_gc_roots_still_omits_stackmap_section() {
    let dir = make_temp_dir("minimal_main_obj_with_live_gc_roots_still_omits_stackmap_section");
    let output = dir.join("main.o");

    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun keepAlive(x: Any): Unit {
}

fun main(): Unit {
    val keep: Any = 1
    // 手动触发一次 GC（调用点应被 statepoint pipeline 产出 stackmap record）。
    __scoop_gc_collect()
    // 显式使用 keep，确保其在 collect 调用点是 live（应出现在 roots locations 后缀）。
    keepAlive(keep)
}
"#,
    );
    let session = Session::new().unwrap();
    emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

    let bytes = std::fs::read(&output).unwrap();
    let obj = object::File::parse(&*bytes).expect("failed to parse object file");
    assert!(
        !object_contains_stackmap_section(&obj),
        "default explicit mode should omit stackmap sections even when a live GC root crosses collect"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(super) fn default_explicit_mode_omits_statepoint_intrinsics_and_gc_strategy() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    return 0
}
"#,
    );
    let session = Session::new().unwrap();

    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let (target_machine, _target_info) =
        target::host_target_machine_with_opt_level(OptLevel::O0).unwrap();
    run_pass_pipeline(&module, &target_machine, OptLevel::O0).unwrap();

    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("scoop_alloc_typed"),
        "expected allocation path to remain present in LLVM IR"
    );
    assert!(
        !ir.contains(r#"gc "statepoint-example""#),
        "default explicit mode should not tag functions with the LLVM statepoint GC strategy"
    );
    assert!(
        !ir.contains("llvm.experimental.gc.statepoint")
            && !ir.contains("llvm.experimental.stackmap"),
        "default explicit mode should not emit statepoint/stackmap intrinsics"
    );
}

#[test]
pub(super) fn stackmap_statepoint_smoke_helper_opt_in_reenables_stackmap_pipeline() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    return __scoop_stackmap_statepoint_smoke()
}
"#,
    );
    let session = Session::new().unwrap();

    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let (target_machine, _target_info) =
        target::host_target_machine_with_opt_level(OptLevel::O0).unwrap();
    run_pass_pipeline(&module, &target_machine, OptLevel::O0).unwrap();

    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains(r#"gc "statepoint-example""#),
        "explicit stackmap smoke helper should opt its caller back into the LLVM statepoint GC strategy"
    );
    assert!(
        ir.contains("@llvm.experimental.gc.statepoint")
            && ir.contains("@scoop_test_stackmap_statepoint_smoke"),
        "explicit stackmap smoke helper should still lower to a real managed statepoint call"
    );
}

#[test]
pub(super) fn stackmap_statepoint_smoke_helper_emits_stackmap_section_when_requested() {
    let dir = make_temp_dir("stackmap_statepoint_smoke_helper_emits_stackmap_section");
    let output = dir.join("main.o");

    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    return __scoop_stackmap_statepoint_smoke()
}
"#,
    );
    let session = Session::new().unwrap();
    emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

    let bytes = std::fs::read(&output).unwrap();
    let obj = object::File::parse(&*bytes).expect("failed to parse object file");
    assert!(
        object_contains_stackmap_section(&obj),
        "explicit stackmap smoke helper should still be able to emit a stackmap section on demand"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(super) fn minimal_main_asm_written_is_non_empty() {
    let dir = make_temp_dir("minimal_main_asm_written_is_non_empty");
    let output = dir.join("main.s");

    let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
    let session = Session::new().unwrap();
    emit_minimal_main_asm_to_file(&session, &source, &output).unwrap();

    let size = std::fs::metadata(&output).unwrap().len();
    assert!(size > 0, "assembly 文件不应为空");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(super) fn managed_function_emits_explicit_root_frame_descriptor() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun keep(name: String): String {
    __scoop_gc_collect()
    return name
}

fun main() {
    println(keep("hi"))
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let keep_ir = function_ir_matching(
        &ir,
        "managed string helper with explicit root descriptor",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && function.contains("@scoop_gc_collect_safepoint")
                && function.contains("ret ptr addrspace(1)")
        },
    );
    let keep_desc = function_ir_explicit_root_descriptor(keep_ir)
        .expect("managed function should publish an explicit root descriptor");
    let keep_offsets = explicit_root_descriptor_offsets_symbol(&ir, keep_desc)
        .expect("managed function descriptor should reference an offsets table");

    assert!(
        llvm_ir_defines_global(&ir, keep_desc),
        "expected managed function descriptor global\n{ir}"
    );
    assert!(
        llvm_ir_global_definition(&ir, keep_offsets)
            .is_some_and(|line| line.contains("internal constant [1 x i32]")),
        "keep() 应发布唯一的合并 root（参数即返回值）\n{ir}"
    );
    assert!(
        llvm_ir_global_definition(&ir, keep_offsets)
            .is_some_and(|line| { line.contains("internal constant [1 x i32] [i32 16]") }),
        "keep() 的显式 root frame 偏移应从 header 后开始并覆盖唯一 home slot\n{ir}"
    );
}
