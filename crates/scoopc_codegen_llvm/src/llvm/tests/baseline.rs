//! Frontend baseline / minimal-main / single-file / via-mir / call-contract / value-box / intrinsic / ctor / extern-global tests.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn minimal_main_ir_contains_main_and_ret0() {
    let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    // `main` 为 C ABI：`i32 @main(i32 argc, i8** argv)`（inkwell/LLVM 版本可能影响参数命名）。
    assert!(ir.contains("define i32 @main("));
    assert!(
        ir.contains("call void @scoop_runtime_init()"),
        "生成的 main 应调用 scoop_runtime_init"
    );
    assert!(
        !ir.contains("@scoop_entry_argv_array"),
        "零参数 main 不应接入 entry argv helper"
    );
    assert!(ir.contains("ret i32 0"));
    assert!(ir.contains("target datalayout ="));
    assert!(ir.contains("target triple ="));
}

#[test]
pub(super) fn minimal_main_ir_with_array_string_args_calls_entry_argv_helper() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(args: Array<String>): Int {
    return args.size()
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("define i32 @main(i32 %argc, ptr %argv)"),
        "entry main 应继续保留 C ABI `main(argc, argv)` 入口，实际 IR:\n{ir}"
    );
    assert!(
        ir.contains("call void @scoop_runtime_init()"),
        "生成的 main 应调用 scoop_runtime_init"
    );
    assert!(
        ir.contains("@scoop_entry_argv_array(i32 %argc, ptr %argv)"),
        "`main(args: Array<String>)` 应通过 runtime helper 把完整 argv 注入到程序边界，实际 IR:\n{ir}"
    );
}

#[test]
pub(super) fn default_single_file_ir_helper_lowers_handle_main_without_hir_fallback() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000_single_file_handle_main_stage.scoop",
        r#"
package a

import scoop.core.Raise

fun main(): Int {
    return handle {
        Raise.raise(1)
        0
    } on {
        Raise.raise(e) -> 2
    }
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source)
        .expect("默认单文件 helper 应走 LLVM stage，而不是命中已删除的 HIR handle lowering");

    assert!(ir.contains("define i32 @main("));
    assert!(
        function_ir_count_matching(&ir, |header, _| header.contains("resume")) >= 1
            && ir.contains("handle_saved_ctx")
            && ir.contains("handle_direct_exit_ctx_clear_saved")
            && ir.contains("%scoop.runtime.ScoopEffectHandlerNode")
            && ir.contains("%scoop.runtime.ScoopEffectCtx"),
        "默认单文件 helper 应继续产出 handle/state-machine lowering，而不是回退到已删除的 HIR handle lowering 或只剩空壳 C main:\n{ir}"
    );
}

#[test]
pub(super) fn single_file_frontend_keeps_distinct_effect_row_generic_instances() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000e2c_single_file.scoop",
        r#"
package fixtures.t5000e2c

import scoop.core.*

effect Boom {
    fun boom(): Int
}

effect Zap {
    fun zap(): Int
}

fun <T, eff E> id(x: T): T / E {
    return x
}

fun <T, eff E> wrap(x: T): T / E {
    return id<T, eff E>(x)
}

private fun entry(): Int / (Boom + Zap) {
    val a = wrap<Int, eff Boom>(1)
    val b = wrap<Int, eff Zap>(2)
    return a + b
}

fun main(): Int / Pure! {
    val thunk: () -> Int / (Boom + Zap) = entry
    return 0
}
"#,
    );
    let session = Session::new().unwrap();
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let materialized = &codegen_unit.lowering.materialized_mir;
    let callable_view = codegen_unit.lowering.materialized_mir.callable_view();
    let materialized_fun_fqns = materialized
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::mir::Item::Fun(fun) if fun.body.is_some() => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let lowered_fun_fqns = codegen_unit
        .lowering
        .lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    for fqn in [
        "fixtures.t5000e2c.wrap::<Int, eff fixtures.t5000e2c.Boom>",
        "fixtures.t5000e2c.wrap::<Int, eff fixtures.t5000e2c.Zap>",
        "fixtures.t5000e2c.id::<Int, eff fixtures.t5000e2c.Boom>",
        "fixtures.t5000e2c.id::<Int, eff fixtures.t5000e2c.Zap>",
    ] {
        assert!(
            lowered_fun_fqns.contains(&fqn),
            "single-file frontend lowering 应保留实例 `{fqn}`，实际函数集合为: {lowered_fun_fqns:?}"
        );
        assert!(
            materialized_fun_fqns.contains(&fqn),
            "single-file frontend 应保留实例 `{fqn}` 的 materialized MIR body，实际 MIR 函数集合为: {materialized_fun_fqns:?}"
        );
        let root = callable_view
            .callable(fqn)
            .expect("callable view 应能直接查询 single-file frontend 的 root body");
        let owner = callable_view
            .owner_of_callable(fqn)
            .expect("callable view 应能从 root body 反查所属实例");
        let family = callable_view
            .instance(owner)
            .expect("callable view 应能从实例读取 canonical family");
        assert_eq!(root.fqn, fqn);
        assert_eq!(family.root_fqn(), fqn);
        assert!(
            family.summary().body_known,
            "有 body 的 root callable 应在 canonical view 中携带 body-known summary"
        );
    }
    assert_eq!(
        callable_view.instances().count(),
        materialized.instance_keys.len(),
        "callable view 应覆盖 single-file frontend 保留的全部实例"
    );
    for family in callable_view.instances() {
        assert!(
            family.summary().body_known == family.root_body().is_some(),
            "canonical callable view 中的 body-known 应与 root body 是否存在一致：{}",
            family.root_fqn()
        );
    }
}

#[test]
pub(super) fn via_mir_direct_interface_default_call_is_not_reinterpreted_as_itable_dispatch() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000gr_interface_dispatch.scoop",
        r#"
package fixtures.t5000gr

import scoop.core.*

interface Ping {
    fun ping(): Int {
        return 7
    }
}

class Box() : Ping

fun <T> use(x: T): Int where T: Ping {
    return x.ping()
}

fun main(): Int {
    return use(Box())
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module_with_opt_level(&session, &source, &context, OptLevel::O2)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        !ir.contains("call_itable"),
        "via-MIR frontend 已把 exact interface dispatch 去虚化为 direct call，backend 不应再按接口 owner FQN 回退成 itable call:\n{ir}"
    );
    let ping_body = function_ir_matching(
        &ir,
        "AbiMangler interface default method body",
        |header, _| header.contains("@__scoop_abi0_fun__fixtures_t5000gr_Ping_ping__h"),
    );
    let ping_symbol = llvm_function_symbol_name(ping_body);
    assert!(
        ping_symbol != "fixtures.t5000gr.Ping.ping"
            && stable_id_symbol_is_exported_abi_fun(ping_symbol),
        "default interface method 应发布到 authoritative ABI namespace，而不是保留 raw callable symbol，实际 symbol: {ping_symbol}"
    );
    assert_eq!(
        function_ir_count_matching(&ir, |header, _| {
            header.contains("@__scoop_abi0_fun__fixtures_t5000gr_Ping_ping__h")
        }),
        1,
        "default interface method 应只发布一个 authoritative ABI symbol，避免不同声明路径各算各的 hash:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("@__scoop_priv0__itable_methods__h")
                && line.contains(&format!("@{ping_symbol}"))
        }),
        "itable method table 应引用与函数定义相同的 authoritative ABI symbol，而不是另一条 declaration path 重新算出来的名字:\n{ir}"
    );
}

#[test]
pub(super) fn llvm_call_contract_lowering() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/cg_t03_call_contracts.scoop",
        r#"
package fixtures.cgt03

import scoop.core.*

interface Ping {
    fun ping(): Int {
        return 7
    }
}

class Box(val value: Int) : Ping

fun read(x: Ping): Int {
    return x.ping()
}

fun main(): Int {
    val b: Box = Box(value = 3)
    val platform: Platform = getPlatform()
    val bytes: Int = sizeOf(1)
    return read(b) + b.value + bytes
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module(&session, &source, &context)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        ir.contains("@__scoop_abi0_fun__fixtures_cgt03_Ping_ping__h"),
        "interface default method slot should keep the selected default implementation:\n{ir}"
    );
    assert!(
        ir.contains("itable_lookup") && ir.contains("load_itable_fn"),
        "interface call through Ping should lower through the authoritative itable lookup path:\n{ir}"
    );
    assert!(
        !ir.contains("scoop.core.getPlatform"),
        "getPlatform should lower to a Platform literal, not a declaration-only intrinsic call:\n{ir}"
    );
}

#[test]
pub(super) fn value_box_interface_itable_points_directly_to_struct_body_method_symbol() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/p4_t01b_value_box_itable.scoop",
        r#"
package fixtures.p4t01b

import scoop.core.*

interface IFace {
    fun m(): Int
}

struct Counter(val base: Int) : IFace {
    fun m(): Int {
        return this.base + 2
    }
}

fun read(it: IFace): Int {
    return it.m()
}

fun main(): Int {
    return read(Counter { base: 40 })
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let method_ir =
        function_ir_matching(&ir, "value-box user struct interface impl", |header, _| {
            header.contains("@__scoop_abi0_fun__fixtures_p4t01b_Counter_m__h")
        });
    let method_symbol = llvm_function_symbol_name(method_ir);

    assert!(
        ir.lines().any(|line| {
            line.contains("@__scoop_priv0__itable_methods__h")
                && line.contains(&format!("@{method_symbol}"))
        }),
        "value-box itable method table 应直接引用 user struct body method symbol，而不是 box thunk:\n{ir}"
    );
    assert!(
        !ir.contains("itable_dynamic_entry"),
        "plain value-box interface dispatch 不应为 struct receiver 发布 itable thunk shell:\n{ir}"
    );
}

#[test]
pub(super) fn intrinsic_value_box_interface_itable_points_directly_to_struct_body_method_symbol() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual_trusted_syslib(
        "<mem>/p4_t01c_intrinsic_value_box_itable.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.p4t01c

import scoop.core.*

interface DummyIface {
    fun m(): Int
}

@Intrinsic
struct Dummy() : DummyIface {
    override fun m(): Int {
        return 41
    }
}

fun read(it: DummyIface): Int {
    return it.m()
}

fun main(): Int {
    val value: DummyIface = Dummy()
    return read(value)
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let method_ir = function_ir_matching(
        &ir,
        "intrinsic value-box struct interface impl",
        |header, _| header.contains("@__scoop_abi0_fun__fixtures_p4t01c_Dummy_m__h"),
    );
    let method_symbol = llvm_function_symbol_name(method_ir);

    assert!(
        ir.lines().any(|line| {
            line.contains("@__scoop_priv0__itable_methods__h")
                && line.contains(&format!("@{method_symbol}"))
        }),
        "intrinsic value-box itable method table 应直接引用 struct body method symbol，而不是额外 thunk:\n{ir}"
    );
    assert!(
        !ir.contains("itable_dynamic_entry"),
        "intrinsic value-type interface dispatch 不应要求新的 box thunk shell:\n{ir}"
    );
}

#[test]
pub(super) fn overlay_core_intrinsic_array_methods_lower_through_ordinary_generic_class_path() {
    let repo_root = stable_id_repo_root();
    let fixture = repo_root.join(
        "tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.scoop",
    );
    let overlay_root = repo_root.join(
        "tests/fixtures/build/intrinsic_sysroot_overlay_array_mutablearray_body_methods_basic.sysroot",
    );
    let source = SourceFile::load(&fixture).unwrap();
    let session =
        Session::with_options(SessionOptions::new().with_sysroot_overlay(overlay_root.clone()))
            .unwrap();

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let array_token_ir = function_ir_matching(&ir, "overlay Array.token body", |_, function| {
        let symbol = llvm_function_symbol_name(function);
        stable_id_symbol_is_exported_abi_fun(symbol)
            && stable_id_symbol_mentions_fqn(symbol, "scoop.core.Array.token")
    });
    let mutable_echo_ir =
        function_ir_matching(&ir, "overlay MutableArray.echo body", |_, function| {
            let symbol = llvm_function_symbol_name(function);
            stable_id_symbol_is_exported_abi_fun(symbol)
                && stable_id_symbol_mentions_fqn(symbol, "scoop.core.MutableArray.echo")
        });

    assert!(
        llvm_function_symbol_name(array_token_ir).contains("scoop_core_Array_token"),
        "overlay Array.token 应作为 ordinary generic class member materialize 为 ABI fun symbol:\n{ir}"
    );
    assert!(
        llvm_function_symbol_name(mutable_echo_ir).contains("scoop_core_MutableArray_echo"),
        "overlay MutableArray.echo 应作为 ordinary generic class member materialize 为 ABI fun symbol:\n{ir}"
    );
    assert!(
        overlay_root.is_dir(),
        "overlay fixture companion dir should exist: {}",
        overlay_root.display()
    );
}

#[test]
pub(super) fn overlay_core_intrinsic_scalar_body_method_call_keeps_receiver_arg() {
    let repo_root = stable_id_repo_root();
    let fixture =
        repo_root.join("tests/fixtures/build/intrinsic_sysroot_overlay_scalar_method_basic.scoop");
    let overlay_root = repo_root
        .join("tests/fixtures/build/intrinsic_sysroot_overlay_scalar_method_basic.sysroot");
    let source = SourceFile::load(&fixture).unwrap();
    let session =
        Session::with_options(SessionOptions::new().with_sysroot_overlay(overlay_root.clone()))
            .unwrap();

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let negate_ir = function_ir_matching(&ir, "overlay Bool.negate body", |_, function| {
        let symbol = llvm_function_symbol_name(function);
        stable_id_symbol_is_exported_abi_fun(symbol)
            && stable_id_symbol_mentions_fqn(symbol, "scoop.core.Bool.negate")
    });
    let negate_symbol = llvm_function_symbol_name(negate_ir);
    let main_ir = function_ir_matching(&ir, "fixture Scoop main body", |_, function| {
        let symbol = llvm_function_symbol_name(function);
        stable_id_symbol_is_exported_abi_fun(symbol)
            && stable_id_symbol_mentions_fqn(symbol, "fixtures.build.main")
    });

    assert!(
        main_ir.lines().any(|line| {
            line.contains(" call ") && line.contains(negate_symbol) && line.contains("i1 ")
        }),
        "Bool.negate direct call should pass the builtin Bool receiver as arg 0:\n{main_ir}"
    );
    assert!(
        overlay_root.is_dir(),
        "overlay fixture companion dir should exist: {}",
        overlay_root.display()
    );
}

#[test]
pub(super) fn overlay_core_intrinsic_scalar_tostring_dispatch_publishes_override_and_default_bodies()
 {
    let repo_root = stable_id_repo_root();
    let fixture = repo_root
        .join("tests/fixtures/build/intrinsic_sysroot_overlay_scalar_tostring_basic.scoop");
    let overlay_root = repo_root
        .join("tests/fixtures/build/intrinsic_sysroot_overlay_scalar_tostring_basic.sysroot");
    let source = SourceFile::load(&fixture).unwrap();
    let session =
        Session::with_options(SessionOptions::new().with_sysroot_overlay(overlay_root.clone()))
            .unwrap();

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    for fqn in [
        "scoop.core.Bool.toString",
        "scoop.core.Char.toString",
        "scoop.core.Float32.toString",
        "scoop.core.Float64.toString",
        "scoop.core.Int.toString",
        "scoop.core.String.toString",
        "scoop.core.ToString.toString",
    ] {
        function_ir_matching(&ir, fqn, |_, function| {
            let symbol = llvm_function_symbol_name(function);
            stable_id_symbol_is_exported_abi_fun(symbol)
                && stable_id_symbol_mentions_fqn(symbol, fqn)
        });
    }
    assert!(
        overlay_root.is_dir(),
        "overlay fixture companion dir should exist: {}",
        overlay_root.display()
    );
}

#[test]
pub(super) fn named_intrinsic_dummy_ir_method_call_does_not_materialize_method_symbol() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual_trusted_syslib(
        "<mem>/p4_t01d_named_intrinsic_dummy_ir.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.p4t01d

import scoop.core.*

@Intrinsic
class Vec<T>(seed: T) {
    @Intrinsic("dummy_ir")
    fun foo(): Int
}

fun main(): Int {
    val vec: Vec<Int> = Vec(1)
    return vec.foo()
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let main_ir = function_ir_matching(
        &ir,
        "P4-T01d named intrinsic dummy_ir main",
        |_, function| {
            stable_id_symbol_mentions_fqn(
                llvm_function_symbol_name(function),
                "fixtures.p4t01d.main",
            )
        },
    );

    assert!(
        maybe_function_ir_matching(&ir, |_, function| {
            stable_id_symbol_mentions_fqn(
                llvm_function_symbol_name(function),
                "fixtures.p4t01d.Vec.foo",
            )
        })
        .is_none(),
        "dummy_ir method call 不应物化 declaration-only method symbol；应直接按 intrinsic 表发 IR:\n{ir}"
    );
    assert!(
        main_ir.contains("41"),
        "dummy_ir method call 应直接在调用点发出常量 IR，而不是保留 method call:\n{main_ir}"
    );
}

#[test]
pub(super) fn named_intrinsic_dummy_runtime_fun_call_lowers_to_runtime_symbol() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual_trusted_syslib(
        "<mem>/p4_t01d_named_intrinsic_dummy_runtime.scoop",
        r#"
@file:AllowIntrinsic

package fixtures.p4t01d

import scoop.core.*

@Intrinsic("dummy_runtime")
fun bar(): Int

fun main(): Int {
    return bar()
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_test_named_intrinsic_dummy_runtime"),
        "dummy_runtime 应直接声明/调用测试 runtime 符号:\n{ir}"
    );
    assert!(
        maybe_function_ir_matching(&ir, |_, function| {
            stable_id_symbol_mentions_fqn(
                llvm_function_symbol_name(function),
                "fixtures.p4t01d.bar",
            )
        })
        .is_none(),
        "dummy_runtime 不应物化 declaration-only Scoop 函数符号:\n{ir}"
    );
}

#[test]
pub(super) fn intrinsic_nominal_body_method_fixtures_do_not_introduce_by_name_compiler_paths() {
    let compiler_root = stable_id_repo_root().join("crates/scoopc/src");
    let mut files = Vec::new();
    stable_id_collect_audit_files("crates/scoopc/src", &compiler_root, &mut files);

    let forbidden_needles = ["DummyIface", "DummyIter"];
    let mut hits = Vec::new();
    for (_, path) in files {
        if path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (line_number, line) in contents.lines().enumerate() {
            if forbidden_needles.iter().any(|needle| line.contains(needle)) {
                hits.push(format!(
                    "{}:{}: {}",
                    stable_id_relative_repo_path(&path),
                    line_number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "P4-T01c 不应为 fixture interface 名字新增编译器按名分支；命中位置:\n{}",
        hits.join("\n")
    );
}

#[test]
pub(super) fn llvm_ctor_default_arg_contract_lowering() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/cg_t03_ctor_default_args.scoop",
        r#"
package fixtures.cgt03

class Pair(val first: Int = 7, val second: Int)

fun main(): Int {
    val pair: Pair = Pair(second = 5)
    return pair.first + pair.second
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).expect(
        "defaulted class ctor call should lower through the published ordered-args contract",
    );
    assert!(
        ir.contains("define i32 @main("),
        "ctor default-arg build should still produce a main entry:\n{ir}"
    );
}

#[test]
pub(super) fn llvm_extern_global() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/cg_t07_extern_global.scoop",
        r#"
package fixtures.cgt07

import scoop.core.*

@Extern(name = "scoop_test_extern_global_counter")
var NativeCounter: Int

@ThreadLocal
@Extern(name = "scoop_test_extern_tls_counter")
var NativeTls: Int

fun main(): Int {
    @Unsafe do { NativeCounter = 1 }
    @Unsafe do { NativeTls = NativeCounter + 1 }
    val value: Int = @Unsafe do { NativeTls }
    return value - 2
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module(&session, &source, &context)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        ir.contains("@scoop_test_extern_global_counter = external global i64"),
        "extern global should lower to the published C symbol instead of a Scoop init root:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_test_extern_tls_counter = external thread_local global i64"),
        "@ThreadLocal @Extern var should preserve TLS storage in LLVM:\n{ir}"
    );
    assert!(
        !ir.contains("fixtures.cgt07.NativeCounter"),
        "extern global codegen must not synthesize ordinary top-level storage from the FQN:\n{ir}"
    );
}

#[test]
pub(super) fn llvm_top_level_eager_init_does_not_call_object_once_runtime() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/cg_p6_top_level_eager_init_no_once.scoop",
        r#"
package fixtures.p6_no_once

import scoop.core.*

fun seed(): Int {
    return 7
}

val Value: Int = seed()

@Global
var Mutable: Int = Value + 1

fun main(): Int {
    return Value + Mutable
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module(&session, &source, &context)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        !ir.contains("scoop_once_begin") && !ir.contains("scoop_once_end"),
        "top-level eager init must not reuse the object once runtime path:\n{ir}"
    );
}

#[test]
pub(super) fn llvm_object_singleton_init_keeps_object_once_runtime() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/cg_p6_object_once_stays_object_only.scoop",
        r#"
package fixtures.p6_object_once

object Holder {
    val value: Int = 41
}

fun main(): Int {
    return Holder.value + Holder.value
}
"#,
    );

    let context = Context::create();
    let ir = build_minimal_main_module(&session, &source, &context)
        .unwrap()
        .print_to_string()
        .to_string();
    assert!(
        ir.contains("scoop_once_begin") && ir.contains("scoop_once_end"),
        "object singleton init should remain on the object once runtime path:\n{ir}"
    );
}

#[test]
pub(super) fn float_builtin_types_lower_to_llvm_scalars() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

@Extern(name = "scoop_test_seed64")
fun seed64(): Float64

@Extern(name = "scoop_test_seed32")
fun seed32(): Float32

fun id64(x: Float64): Float64 {
    return x
}

fun id32(x: Float32): Float32 {
    return x
}

fun choose(flag: Bool, left: Float64, right: Float64): Float64 {
    if (flag) {
        return left
    }
    return right
}

fun main() {
    val a64: Float64 = @Unsafe do { seed64() }
    val a32: Float32 = @Unsafe do { seed32() }
    val b64: Float64 = id64(a64)
    val b32: Float32 = id32(a32)
    val c64: Float64 = choose(true, b64, a64)
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let id64_ir = function_ir_matching(&ir, "Float64 identity function", |header, function| {
        !header.contains("@main(")
            && header.contains("double @")
            && header.contains("(double")
            && !function.contains("br i1")
            && function.contains("ret double")
    });
    let id32_ir = function_ir_matching(&ir, "Float32 identity function", |header, function| {
        !header.contains("@main(")
            && header.contains("float @")
            && header.contains("(float")
            && !function.contains("br i1")
            && function.contains("ret float")
    });
    let choose_ir = function_ir_matching(&ir, "Float64 chooser", |header, function| {
        !header.contains("@main(")
            && header.contains("double @")
            && header.contains("(i1")
            && function.contains("br i1")
            && function.contains("ret double")
    });
    let choose_symbol = llvm_function_symbol_name(choose_ir);

    assert!(
        id64_ir
            .lines()
            .next()
            .is_some_and(|header| { header.contains("double @") && header.contains("(double") }),
        "Float64 should lower to LLVM double in function signatures"
    );
    assert!(
        id32_ir
            .lines()
            .next()
            .is_some_and(|header| { header.contains("float @") && header.contains("(float") }),
        "Float32 should lower to LLVM float in function signatures"
    );
    assert!(
        ir.contains("declare double @scoop_test_seed64()"),
        "extern Float64 function should keep double ABI"
    );
    assert!(
        ir.contains("declare float @scoop_test_seed32()"),
        "extern Float32 function should keep float ABI"
    );
    assert!(
        function_ir_count_matching(&ir, |header, function| {
            !header.contains("@main(") && function_ir_calls_symbol(function, choose_symbol)
        }) >= 1,
        "Float64 return values should stay on the LLVM scalar path through calls"
    );
}

#[test]
pub(super) fn float_builtin_methods_lower_to_runtime_calls_and_hash_bits() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

@Extern(name = "scoop_test_seed64")
fun seed64(): Float64

@Extern(name = "scoop_test_seed32")
fun seed32(): Float32

fun main() {
    val a64: Float64 = @Unsafe do { seed64() }
    val a32: Float32 = @Unsafe do { seed32() }

    val s64: String = a64.toString()
    val s32: String = a32.toString()
    val i64: Int = a64.toInt()
    val i32: Int = a32.toInt()
    val h64: Int = a64.hash()
    val h32: Int = a32.hash()
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_float64_to_string("),
        "Float64.toString should declare the runtime conversion symbol"
    );
    assert!(
        ir.contains("@scoop_float32_to_string("),
        "Float32.toString should declare the runtime conversion symbol"
    );
    assert!(
        ir.contains("@scoop_float64_to_int("),
        "Float64.toInt should declare the runtime conversion symbol"
    );
    assert!(
        ir.contains("@scoop_float32_to_int("),
        "Float32.toInt should declare the runtime conversion symbol"
    );
    assert!(
        ir.contains("f64_hash_bits"),
        "Float64.hash should lower via float-bit reinterpretation"
    );
    assert!(
        ir.contains("f32_hash_bits"),
        "Float32.hash should lower via float-bit reinterpretation"
    );
}

#[test]
pub(super) fn float_literals_lower_to_arithmetic_comparisons_and_narrowing() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

val topWide: Float64 = 1.25
val topNarrow: Float32 = 1.5

fun main() {
    val wideBase: Float64 = 1.25
    val narrowBase: Float32 = 1.5
    val wideSum: Float64 = wideBase + 2.75
    val narrowSum: Float32 = narrowBase + 0.5f
    val narrowRem: Float32 = narrowSum % 1.5f
    val absorbed: Float32 = 1.5
    val negWide: Float64 = -wideBase
    val lt: Bool = wideSum < 10.0
    val eq: Bool = narrowBase == 1.5
    val ne: Bool = narrowBase != 2.5
    val text: String = 1.25e2.toString()
    val whole: Int = 3.75.toInt()
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("fadd double"),
        "Float64 arithmetic should lower via LLVM floating-point add"
    );
    assert!(
        ir.contains("fadd float"),
        "Float32 arithmetic should lower via LLVM floating-point add"
    );
    assert!(
        ir.contains("frem float"),
        "Float32 remainder should lower via LLVM floating-point remainder"
    );
    assert!(
        ir.contains("float 1.500000e+00"),
        "Unsuffixed Float literals in Float32 contexts should lower as LLVM float constants"
    );
    assert!(
        ir.contains("fcmp olt double"),
        "Float comparisons should use ordered LLVM floating-point predicates"
    );
    assert!(
        ir.contains("fcmp oeq float") || ir.contains("fcmp oeq double"),
        "Float equality should use ordered equality for NaN-sensitive semantics"
    );
    assert!(
        ir.contains("fcmp une float") || ir.contains("fcmp une double"),
        "Float inequality should treat NaN as not-equal"
    );
    assert!(
        ir.contains("fneg double"),
        "Unary Float negation should lower to LLVM floating-point negation"
    );
    assert!(
        ir.contains("@scoop_float64_to_string("),
        "Float literal member calls should reuse Float.toString runtime lowering"
    );
    assert!(
        ir.contains("@scoop_float64_to_int("),
        "Float literal member calls should reuse Float.toInt runtime lowering"
    );
}

#[test]
pub(super) fn lowered_call_results_keep_concrete_types_for_local_bindings() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun id(x: Int): Int { return x }

fun main() {
    val n = id(1)
    val mag = (-2.5).abs()
    val inf = (1.0 / 0.0).isInfinite()

    println(n.toString())
    println(mag.toString())
    println(inf.toString())
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_int_to_string("),
        "Unannotated local Int call results should keep Int through sysroot toString lowering"
    );
    assert!(
        ir.contains("@scoop_float64_to_string("),
        "Unannotated local Float call results should keep Float64 through sysroot toString lowering"
    );
    assert!(
        ir.contains("@scoop_bool_to_string("),
        "Unannotated local Bool call results should use the scoop ABI Bool.toString runtime helper"
    );
}

#[test]
pub(super) fn lowered_hir_codegen_accepts_materialized_generic_sysroot_direct_calls() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package fixtures.t5000e3d

import scoop.core.*

fun main(): Int {
    println(1)
    return 0
}
"#,
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    assert!(
        ir.contains("@scoop_println"),
        "materialized generic sysroot direct-call should print through compiled sysroot println"
    );
    assert!(
        ir.contains("scoop_core_println") && ir.contains("scoop_core_Int_toString"),
        "compiled sysroot println<Int> should call the Int.toString body instead of a print builtin bypass"
    );
}

#[test]
pub(super) fn class_init_order_fixture_codegen_accepts_materialized_generic_sysroot_direct_calls() {
    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/class_init_order_primary_secondary_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_println"),
        "class/object/init helper 中的 concrete generic direct call 应继续复用 materialized generic sysroot direct-call lowering\n{ir}"
    );
}
