#[cfg(test)]
mod clayout_tests {
    use super::*;

    #[test]
    fn clayout_packed_struct_has_expected_field_offsets() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = Packed { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let data_layout = module.get_data_layout();
        let target_data = TargetData::create(data_layout.as_str().to_str().unwrap());

        let packed = context
            .get_struct_type("fixtures.clayout.Packed")
            .expect("missing llvm struct type for fixtures.clayout.Packed");
        assert!(
            packed.is_packed(),
            "expected @CLayout(packed=1) struct to be packed in LLVM"
        );
        assert_eq!(
            target_data.offset_of_element(&packed, 1).unwrap(),
            1,
            "expected second field offset to be 1 for packed struct"
        );
    }

    #[test]
    fn clayout_aligned_struct_sets_alloca_alignment() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_aligned.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(aligned: 16, packed: 1)
struct AlignedPacked(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = AlignedPacked { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let ir = module.print_to_string().to_string();

        assert!(
            ir.contains(
                "__scoop_composite_transport_desc__inline__fixtures_clayout_AlignedPacked__Struct"
            ) && ir.contains("i64 16, i64 16"),
            "@CLayout(aligned=16, packed=1) 应把 composite transport 物理布局发布为 size=16 / align=16\n{ir}"
        );
    }

    #[test]
    fn clayout_packed_field_load_uses_align_1() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed_field_load.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s: Packed = Packed { a: a0, b: b0 }
    val x: Int64 = s.b
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let ir = module.print_to_string().to_string();

        assert!(
            ir.contains(
                "__scoop_composite_transport_desc__inline__fixtures_clayout_Packed__Struct"
            ) && ir.contains("i64 9, i64 1"),
            "@CLayout(packed=1) 应继续把 composite transport 物理布局发布为 size=9 / align=1\n{ir}"
        );
    }
}

use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::ast;
use crate::hir;
use crate::opt::OptLevel;
use crate::parser::parse_file;
use crate::resolve::Index;
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::TypeStore;
use inkwell::context::Context;
use inkwell::targets::TargetData;
use object::Object;
use object::ObjectSection;

fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("scoopc_{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn minimal_main_ir_contains_main_and_ret0() {
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
fn minimal_main_ir_with_array_string_args_calls_entry_argv_helper() {
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
fn default_single_file_ir_helper_lowers_handle_main_without_hir_fallback() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000_single_file_handle_main_stage.scoop",
        r#"
package a

import scoop.core.Raise

fun main(): Int {
    return handle {
        Raise.raise(1)
        0
    } with {
        Raise.raise(e) -> 2
    }
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).expect(
        "默认单文件 helper 应走 refactor LLVM stage，而不是命中已删除的 HIR handle lowering",
    );

    assert!(ir.contains("define i32 @main("));
    }

#[test]
fn single_file_frontend_keeps_distinct_effect_row_generic_instances() {
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
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("single-file frontend 应保留 materialized MIR");
    let callable_view = codegen_unit
        .lowered
        .materialized_callable_view()
        .expect("single-file frontend 应暴露 materialized callable view");
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
        .lowered
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
fn via_mir_direct_interface_default_call_is_not_reinterpreted_as_itable_dispatch() {
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
    assert!(
        ir.contains("@fixtures.t5000gr.Ping.ping"),
        "default interface method 的 direct target 应继续保留在 IR 中，实际 IR:\n{ir}"
    );
}

#[test]
fn refactor_llvm_call_contract_lowering() {
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
        ir.contains("@fixtures.cgt03.Ping.ping"),
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
fn refactor_llvm_extern_global() {
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
fn float_builtin_types_lower_to_llvm_scalars() {
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

    assert!(
        ir.contains("define double @a.id64("),
        "Float64 should lower to LLVM double in function signatures"
    );
    assert!(
        ir.contains("define float @a.id32("),
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
        ir.contains("call double @a.choose("),
        "Float64 return values should stay on the LLVM scalar path through calls"
    );
}

#[test]
fn float_builtin_methods_lower_to_runtime_calls_and_hash_bits() {
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
fn float_literals_lower_to_arithmetic_comparisons_and_narrowing() {
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
fn lowered_call_results_keep_concrete_types_for_local_bindings() {
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

    let mut ast = parse_file(&source).unwrap();
    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((&source, &ast));
        Index::build(&pairs).unwrap()
    };

    let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    crate::typecheck::check_file_annotations(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_type_refs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_exprs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        unit.push((&file.source, &file.ast));
    }
    unit.push((&source, &ast));

    let files_to_lower = vec![(&source, &ast)];
    let _lowered = hir::lower_for_compilation_unit_multi_files(
        &source,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &typecheck_types,
    )
    .unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_int_to_string("),
        "Unannotated local Int call results should keep Int through lowering/codegen"
    );
    assert!(
        ir.contains("@scoop_float64_to_string("),
        "Unannotated local Float call results should keep Float64 through lowering/codegen"
    );
    assert!(
        ir.contains("@scoop_bool_to_string("),
        "Unannotated local Bool call results should keep Bool through lowering/codegen"
    );
}

#[test]
fn lowered_hir_codegen_accepts_materialized_generic_sysroot_direct_calls() {
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

    let mut ast = parse_file(&source).unwrap();
    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((&source, &ast));
        Index::build(&pairs).unwrap()
    };

    let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    crate::typecheck::check_file_annotations(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_type_refs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_exprs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        unit.push((&file.source, &file.ast));
    }
    unit.push((&source, &ast));

    let files_to_lower = vec![(&source, &ast)];
    let lowered = hir::lower_for_compilation_unit_multi_files(
        &source,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &typecheck_types,
    )
    .unwrap();

    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");
    let Some(hir::Stmt {
        kind: hir::StmtKind::Expr(call),
        ..
    }) = body.stmts.first()
    else {
        panic!("expected println statement in lowered main body");
    };
    let hir::ExprKind::Call { callee, .. } = &call.kind else {
        panic!("expected println statement to lower as a direct call");
    };
    let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        panic!("expected println callee to lower as a top-level direct-call target");
    };
    assert!(
        fqn.starts_with("scoop.core.println::<"),
        "HIR should already materialize the generic sysroot direct-call target before LLVM dispatch: {fqn}"
    );

    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    assert!(
        ir.contains("@scoop_println"),
        "materialized generic sysroot direct-call should still route through builtin print lowering"
    );
}

#[test]
fn frontend_codegen_rewrites_string_literal_compare_to_and_concat_to_extension_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered main body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1c_string_literal_direct_calls.scoop",
        r#"
package fixtures.t5000j1c

import scoop.core.*

fun main(): Int {
    val strCmp = "ab".compareTo("ac") < 0
    val strEq = "hi".concat("!") == "hi!"
    return if (strCmp && strEq) { 0 } else { 1 }
}
"#,
    );

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let main = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000j1c.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    let hir::ExprKind::Binary {
        lhs: cmp_lhs,
        op: cmp_op,
        rhs: cmp_rhs,
        ..
    } = &find_local_init(body, "strCmp").kind
    else {
        panic!("strCmp should lower to a binary compare expression");
    };
    assert_eq!(*cmp_op, ast::BinaryOp::Lt);
    assert_top_level_call(cmp_lhs, "scoop.core.compareTo", 2);
    assert!(matches!(
        cmp_rhs.kind,
        hir::ExprKind::Literal(hir::LiteralKind::Int | hir::LiteralKind::SynthInt(0))
    ));

    let hir::ExprKind::Binary {
        lhs: concat_lhs,
        op: concat_op,
        rhs: concat_rhs,
        ..
    } = &find_local_init(body, "strEq").kind
    else {
        panic!("strEq should lower to a binary equality expression");
    };
    assert_eq!(*concat_op, ast::BinaryOp::Eq);
    assert_top_level_call(concat_lhs, "scoop.core.concat", 2);
    assert!(matches!(
        concat_rhs.kind,
        hir::ExprKind::Literal(hir::LiteralKind::String)
    ));
}

#[test]
fn builtin_string_intrinsic_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected_fqn: &str) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
        })
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1d_string_intrinsic_member_calls.scoop",
        r#"
package fixtures.t5000j1d

import scoop.core.*

fun inspect(s: String, idx: Int): Int {
    val len = s.byteLength()
    val byte = s.getByte(idx)
    val slice = @Unsafe do { s.unsafeSliceBytes(0, len) }
    return byte + slice.byteLength()
}

fun main(): Int {
    return inspect("hello", 1)
}
"#,
    );
    let inspect_fqn = "fixtures.t5000j1d.inspect";

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let inspect = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == inspect_fqn => Some(fun),
            _ => None,
        })
        .expect("expected lowered inspect helper");
    let body = inspect.body.as_ref().expect("inspect should have a body");

    assert_top_level_call(find_local_init(body, "len"), "scoop.core.byteLength", 1);
    assert_top_level_call(find_local_init(body, "byte"), "scoop.core.getByte", 2);

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let inspect_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == inspect_fqn)
        .expect("inspect helper should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(inspect_mir),
        "String intrinsic member calls should lower to direct contracts, not FunValue calls"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.byteLength"),
        "materialized MIR should contain a direct call to scoop.core.byteLength"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.getByte"),
        "materialized MIR should contain a direct call to scoop.core.getByte"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.unsafeSliceBytes"),
        "materialized MIR should contain a direct call to scoop.core.unsafeSliceBytes"
    );
}

#[test]
fn builtin_string_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected_fqn: &str) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
        })
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1e_string_member_calls.scoop",
        r#"
package fixtures.t5000j1e

import scoop.core.*

fun inspect(s: String): Int {
    val empty = s.isEmpty()
    val replaced = s.replace("a", "b")
    val code = s.charAt(1)
    val repeated = s.repeat(2)
    val emptyScore = if (empty) { 0 } else { 1 }
    return code + replaced.byteLength() + repeated.byteLength() + emptyScore
}

fun main(): Int {
    return inspect("ab")
}
"#,
    );
    let inspect_fqn = "fixtures.t5000j1e.inspect";

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let inspect = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == inspect_fqn => Some(fun),
            _ => None,
        })
        .expect("expected lowered inspect helper");
    let body = inspect.body.as_ref().expect("inspect should have a body");

    assert_top_level_call(find_local_init(body, "empty"), "scoop.core.isEmpty", 1);
    assert_top_level_call(find_local_init(body, "replaced"), "scoop.core.replace", 3);
    assert_top_level_call(find_local_init(body, "code"), "scoop.core.charAt", 2);
    assert_top_level_call(find_local_init(body, "repeated"), "scoop.core.repeat", 2);

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let inspect_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == inspect_fqn)
        .expect("inspect helper should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(inspect_mir),
        "String builtin member calls should lower to direct contracts, not FunValue calls"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.isEmpty"),
        "materialized MIR should contain a direct call to scoop.core.isEmpty"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.replace"),
        "materialized MIR should contain a direct call to scoop.core.replace"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.charAt"),
        "materialized MIR should contain a direct call to scoop.core.charAt"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.repeat"),
        "materialized MIR should contain a direct call to scoop.core.repeat"
    );
}

#[test]
fn builtin_string_trim_indent_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_direct_call_count(fun: &crate::mir::FunDecl, expected_fqn: &str) -> usize {
        let Some(body) = &fun.body else {
            return 0;
        };
        body.blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
            .count()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/string_trim_indent_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let main = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.name == "main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    assert_top_level_call(find_local_init(body, "s"), "scoop.core.trimIndent", 1);
    assert_top_level_call(find_local_init(body, "again"), "scoop.core.trimIndent", 1);

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(main_mir),
        "runtime String.trimIndent() member calls should lower to direct contracts, not FunValue calls"
    );
    assert_eq!(
        mir_fun_direct_call_count(main_mir, "scoop.core.trimIndent"),
        2,
        "materialized MIR should contain exactly two direct calls to scoop.core.trimIndent"
    );
}

#[test]
fn top_level_generic_named_args_keep_canonical_param_order_in_pass_mir() {
    fn direct_call_arg_local_names(fun: &crate::mir::FunDecl, expected_fqn: &str) -> Vec<String> {
        let body = fun.body.as_ref().expect("expected MIR body");
        let args = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let crate::mir::Rvalue::Call { kind, args, .. } = value else {
                    return None;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return None;
                };
                (callee_fqn == expected_fqn).then_some(args)
            })
            .unwrap_or_else(|| panic!("expected direct call to `{expected_fqn}`"));
        args.iter()
            .map(|arg| match &arg.value {
                crate::mir::Operand::Local(local) => body.locals[local.as_u32() as usize]
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("l{}", local.as_u32())),
                other => panic!("expected local call arg for `{expected_fqn}`, actual: {other:?}"),
            })
            .collect()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend should keep materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");

    assert_eq!(
        direct_call_arg_local_names(main_mir, "pick::<Int>"),
        vec!["__call_arg_1".to_string(), "__call_arg_0".to_string()],
        "materialized MIR should preserve the HIR canonical param order for named args"
    );
}

#[test]
fn callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir() {
    fn call_arg_spans_at_stmt(
        fun: &crate::mir::FunDecl,
        stmt_span: crate::span::Span,
    ) -> Vec<crate::span::Span> {
        let body = fun.body.as_ref().expect("expected MIR body");
        let args = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let crate::mir::Rvalue::Call { args, .. } = value else {
                    return None;
                };
                (stmt.span == stmt_span).then_some(args)
            })
            .unwrap_or_else(|| panic!("expected call statement at span {stmt_span:?}"));
        args.iter().map(|arg| arg.span).collect()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend should keep materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");

    assert_eq!(
        call_arg_spans_at_stmt(main_mir, crate::span::Span::new(1442, 1469)),
        vec![
            crate::span::Span::new(1463, 1468),
            crate::span::Span::new(1449, 1450)
        ],
        "named receiver callable-value call should reorder args to receiver-then-a0 in MIR"
    );
    assert_eq!(
        call_arg_spans_at_stmt(main_mir, crate::span::Span::new(1770, 1797)),
        vec![
            crate::span::Span::new(1795, 1796),
            crate::span::Span::new(1781, 1782)
        ],
        "top-level FunPtr named direct call should reorder args to receiver-then-a0 in MIR"
    );
}

#[test]
fn direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3ar_direct_hir_object_init_helper_dep.scoop",
        r#"
package a

object BoomObject {
    init {
        helper()
    }
}

fun helper() {}

fun main(): Int {
    val _obj = BoomObject
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let object_init_ir = function_ir_named(&ir, "__scoop_object_init__a.BoomObject");

    assert!(
        object_init_ir.contains("a.helper"),
        "direct-HIR reachability 也必须保留 object init body 对 helper 的调用:\n{object_init_ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.starts_with("define ") && line.contains("a.helper")),
        "仅由 object init body 触达的 helper 仍必须在模块中拥有定义:\n{ir}"
    );
}

#[test]
fn production_codegen_respects_mir_inlining_opt_level_gate() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000hr_mir_inline_opt_gate.scoop",
        r#"
package fixtures.t5000hr

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    return wrap<Int>(1)
}
"#,
    );
    let wrap_fqn = "fixtures.t5000hr.wrap::<Int>";
    let id_fqn = "fixtures.t5000hr.id::<Int>";

    let o0_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let o0_materialized = o0_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 O0 materialized MIR");
    assert!(
        !o0_materialized
            .pass_view()
            .callable_body_is_overridden(wrap_fqn),
        "O0 不应运行 summary-driven MIR inlining 并覆盖 pass body"
    );
    let o0_wrap = o0_materialized
        .pass_view()
        .callable(wrap_fqn)
        .expect("O0 pass view 仍应可读取 raw materialized body");
    assert!(
        mir_fun_contains_direct_call(o0_wrap, id_fqn),
        "O0 pass view 不应消除 wrap -> id direct call"
    );

    let o2_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let o2_materialized = o2_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 O2 materialized MIR");
    assert!(
        o2_materialized
            .pass_view()
            .callable_body_is_overridden(wrap_fqn),
        "O2 应运行 summary-driven MIR inlining 并覆盖 pass body"
    );
    let o2_wrap = o2_materialized
        .pass_view()
        .callable(wrap_fqn)
        .expect("O2 pass view 应保留 rewritten wrap body");
    assert!(
        !mir_fun_contains_direct_call(o2_wrap, id_fqn),
        "O2 pass view 应消除 wrap -> id direct call"
    );
}

#[test]
fn effect_contract_struct_types_are_registered_for_effect_codegen() {
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
        .expect("refactor effect codegen 应注册共享的 composite transport descriptor 类型");
    assert_eq!(composite_transport.count_fields(), 11);

    let step = context
        .get_struct_type("scoop.refactor.Step__a_go")
        .expect("默认单文件 refactor path 应为 outward callable 注册 Step shell");
    assert_eq!(step.count_fields(), 2);

    let step_complete = context
        .get_struct_type("scoop.refactor.StepComplete__a_go")
        .expect("refactor Step 应发布 complete payload shell");
    assert_eq!(step_complete.count_fields(), 1);

    let resume_vtable = context
        .get_struct_type("scoop.refactor.ResumeVtable__a_go__a_Ping")
        .expect("refactor continuation 应发布 authoritative surface-resume vtable");
    assert_eq!(resume_vtable.count_fields(), 1);

    let continuation = context
        .get_struct_type("scoop.refactor.Continuation__a_go")
        .expect("默认单文件 refactor path 应为 handled perform 注册 continuation object");
    assert!(
        continuation.count_fields() >= 6,
        "continuation object 至少应包含 header/frame/state/one-shot/composed-callee/vtable 字段"
    );

    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_go__k0")
            && ir.contains("@__scoop_refactor_continuation_layout__a_go__type_desc"),
        "默认单文件 refactor path 应继续发布 surface-resume owner dispatch 与 continuation type descriptor:\n{ir}"
    );
}

#[test]
fn indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport() {
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
        ir.contains("refactor_step_payload_insert") && ir.contains("switch i32 %refactor_step_tag"),
        "ordinary callee perform 应通过 refactor Step payload/dispatch lower，而不是依赖旧 perform-slot runtime 入口\n{ir}"
    );
    assert!(
        ir.contains("%scoop.refactor.StepCase__a_go__case0 = type { { ptr addrspace(1), i64 }, ptr addrspace(1) }")
            && ir.contains(
                "insertvalue %scoop.refactor.StepCase__a_go__case0 undef, { ptr addrspace(1), i64 }"
            ),
        "multi-payload perform 应以内联 tuple payload 发布 refactor Step case，而不是丢参或回旧 boxing ABI\n{ir}"
    );
    assert!(
        ir.contains(
            "extractvalue { ptr addrspace(1), i64 } %refactor_boundary_case_payload_payload, 0"
        ) && ir.contains(
            "extractvalue { ptr addrspace(1), i64 } %refactor_boundary_case_payload_payload, 1"
        ),
        "handler binder lowering 应继续按 tuple payload 的两个字段读取 binder，而不是退回单值 transport\n{ir}"
    );
    }

#[test]
fn effectful_closure_dynamic_fallback_uses_schema_aware_carrier_adapter() {
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

    assert!(
        ir.contains("store ptr @__scoop_refactor_closure_step_adapter__a_main__lambda0__")
            && ir.contains("refactor_carrier_to_effectful"),
        "effectful closure carrier 应写入 schema-aware adapter，而不是直接写 owner dynamic entry:\n{ir}"
    );
    assert!(
        !ir.contains("store ptr @__scoop_refactor_closure_dynamic_entry__a_main__lambda0"),
        "closure surface step schema 与 owner step schema 不一致时，不应把 raw owner dynamic entry 直接写进 closure object:\n{ir}"
    );
}

#[test]
fn higher_order_effectful_function_value_uses_schema_aware_carrier_adapter() {
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

    assert!(
        ir.contains("store ptr @__scoop_refactor_plain_adapter__a_choose__lambda0__")
            && ir.contains("store ptr @__scoop_refactor_closure_step_adapter__a_choose__lambda1__"),
        "higher-order callable coercion 应同时保留 pure-branch plain adapter 与 effectful-branch schema-aware carrier adapter:\n{ir}"
    );
    assert!(
        !ir.contains("store ptr @__scoop_refactor_closure_dynamic_entry__a_choose__lambda1"),
        "effectful higher-order branch 不应把 raw owner dynamic entry 直接写进 closure object:\n{ir}"
    );
}

#[test]
fn state_machine_multi_payload_perform_uses_tuple_transport() {
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
        ir.contains("refactor_step_payload_insert") && ir.contains("switch i32 %refactor_step_tag"),
        "state-machine perform 应通过 refactor Step payload/dispatch lower，而不是依赖旧 perform-slot runtime 入口\n{ir}"
    );
    assert!(
        ir.contains("StepCase__a_main__case0"),
        "state-machine multi-payload perform 应以内联 tuple payload 穿过 handle arm，而不是退回旧 boxing ABI\n{ir}"
    );
    assert!(
        ir.contains("refactor_handle_arm_payload_reload") && ir.contains("refactor_payload_field"),
        "state-machine handler binder lowering 应继续按 tuple payload 的两个字段读取 binder\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_main__k")
            && ir.contains("@__scoop_refactor_surface_resume__k3"),
        "Continuation.resume lowering 应改走 published surface-resume owner dispatch，而不是旧 runtime helper 入口\n{ir}"
    );
    let resume_idx = ir
        .find("@__scoop_refactor_surface_resume__k3")
        .expect("expected published surface-resume call in emitted IR");
    let resume_window_start = resume_idx.saturating_sub(500);
    let resume_window_end = std::cmp::min(resume_idx + 2200, ir.len());
    let resume_window = &ir[resume_window_start..resume_window_end];
    assert!(
        resume_window
            .contains("extractvalue %scoop.refactor.Step__schema3 %refactor_resume_step, 0")
            && resume_window.contains("br i1 %refactor_step_is_complete"),
        "surface-resume call return path 应继续按 Step tag dispatch，而不是回答案专用 helper\n{resume_window}"
    );
    assert!(
        ir.contains("refactor_resume_state") && ir.contains("refactor_store_one_shot_gep"),
        "surface-resume path 应继续显式消费 continuation state/one-shot contract\n{resume_window}"
    );
    assert!(
        ir.contains("store i32 %refactor_resume_state")
            || ir.contains("store i32 2, ptr addrspace(1) %refactor_cont_state_gep"),
        "surface-resume return path 应继续把 continuation state 写回 object contract\n{resume_window}"
    );
        }

#[test]
fn cross_call_escape_resume_roots_do_not_degrade_to_poison_in_explicit_frame() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(
            "../../../../tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop"
        ),
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("explicit_root_frame_slot_"),
        "emitted IR should keep refactor-owned roots in explicit frame homes\n{ir}"
    );
    assert!(
        !ir.contains("ptr poison"),
        "cross-call escaped continuation resume roots must not degrade to poisoned spill homes\n{ir}"
    );
}

#[test]
fn direct_effectful_signature_without_outward_effect_skips_tls_check() {
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
    let entry_ir = function_ir_named_any(
        &ir,
        &["@a.entry(", "__scoop_refactor_direct_invoke__a_entry"],
    );

    }

#[test]
fn direct_call_with_uncalled_effectful_higher_order_param_skips_tls_check() {
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
    let entry_ir = function_ir_named_any(
        &ir,
        &["@a.entry(", "__scoop_refactor_direct_invoke__a_entry"],
    );

    }

#[test]
fn closure_call_without_outward_effect_skips_tls_check() {
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
    let entry_ir = function_ir_named_any(
        &ir,
        &["@a.entry(", "__scoop_refactor_direct_invoke__a_entry"],
    );

    }

#[test]
fn direct_call_with_real_outward_effect_uses_wrapper_and_explicit_outcome() {
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
    let entry_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_entry");
    let outward_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_outward");

            assert!(
        ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_entry__k")
            && ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_outward__k"),
        "refactor direct outward path 应继续发布 entry/callee 的 authoritative surface-resume owner dispatch:\n{ir}"
    );
}

#[test]
fn closure_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
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
    let entry_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_entry");

        assert!(
        entry_ir.contains("@__scoop_refactor_direct_invoke__a_entry__lambda0"),
        "当前默认路径会把单次 outward closure thunk 直接绑定到 authoritative lambda entry，而不是回落旧 wrapper/TLS probing:\n{entry_ir}"
    );
}

#[test]
fn effectful_funptr_call_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*
import scoop.unsafe.*

effect Ask {
    fun ask(seed: Int): Int
}

@Extern("scoop_test_get_effectful_funptr")
fun get_effectful_funptr(): FunPtr<() -> Int / (Ask)>

fun entry(): Int / (Ask) {
    val fp: FunPtr<() -> Int / (Ask)> = @Unsafe do { get_effectful_funptr() }
    return @Unsafe do { fp() }
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
    let entry_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_entry");

        assert!(
        entry_ir.contains("refactor_dynamic_funptr_fn = inttoptr i64")
            && entry_ir.contains(
                "refactor_dynamic_call_step = call %scoop.refactor.Step__schema2 %refactor_dynamic_funptr_fn(i64"
            ),
        "effectful FunPtr 调用应直接把 machine-word funptr 还原成 dynamic entry 并返回 Step，而不是回旧 call_funptr helper:\n{entry_ir}"
    );
}

#[test]
fn virtual_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
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
    let helper_ir = function_ir_named_any(
        &ir,
        &["__scoop_refactor_direct_invoke__a_helper", "a.helper"],
    );

    assert!(
        helper_ir.contains("load_vtable_fn")
            && helper_ir.contains("call %scoop.refactor.Step__schema")
            && helper_ir.contains("switch i32 %refactor_step_tag"),
        "默认 virtual-cone path 的 outward vtable helper 应走 refactor Step dispatch，而不是缺失 helper body 或回落旧 wrapper:\n{helper_ir}"
    );
    assert!(
        ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_helper__k"),
        "默认 virtual-cone path 的 outward vtable helper 应继续发布 authoritative surface-resume owner dispatch:\n{ir}"
    );
    }

#[test]
fn interface_call_with_real_outward_effect_uses_explicit_outcome_boundary() {
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
    let helper_ir = function_ir_named_any(
        &ir,
        &["__scoop_refactor_direct_invoke__a_helper", "a.helper"],
    );

    assert!(
        helper_ir.contains("itable_lookup")
            && helper_ir.contains("load_itable_fn")
            && helper_ir.contains("call %scoop.refactor.Step__schema"),
        "默认 virtual-cone path 的 outward itable helper 应走 refactor Step dispatch，而不是缺失 helper body 或回落旧 wrapper:\n{helper_ir}"
    );
    assert!(
        ir.contains("@__scoop_refactor_surface_resume_owner_dispatch__a_helper__k"),
        "默认 virtual-cone path 的 outward itable helper 应继续发布 authoritative surface-resume owner dispatch:\n{ir}"
    );
    }

#[test]
fn object_value_init_access_stays_plain_without_effect_boundary() {
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    }

#[test]
fn object_property_init_access_stays_plain_without_effect_boundary() {
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    }

#[test]
fn top_level_immutable_init_access_stays_plain_without_effect_boundary() {
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    }

#[test]
fn pure_extern_call_does_not_install_effect_boundary() {
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    }

#[test]
fn refactor_class_ctor_uses_concrete_generic_instance_layout() {
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

    let payload_ty = context
        .get_struct_type("scoop.runtime.ClassPayload__a_Box_String_")
        .expect("generic Box<String> constructor should publish a concrete class payload type");
    let fields = payload_ty.get_field_types();
    assert_eq!(
        fields.len(),
        1,
        "Box<String> concrete payload should have the substituted value field"
    );
    assert!(
        fields[0].is_pointer_type(),
        "Box<String>.value should lower as the concrete String GC pointer field, not generic T"
    );
    assert!(
        ir.contains("@__scoop_type_desc_class__a_Box_String_"),
        "constructor allocation should use the concrete Box<String> type descriptor\n{ir}"
    );
    assert!(
        !ir.contains("@__scoop_type_desc_class__a_Box ="),
        "generic constructor must not allocate with the raw Box<T> descriptor\n{ir}"
    );
}

#[test]
fn indirect_gc_aggregate_param_syncs_explicit_frame_home_slot_on_entry() {
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
    let keep_ir = function_ir_named(&ir, "@a.keep(");

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
fn single_file_minimal_ir_includes_compilable_sysroot_string_helpers() {
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

    assert!(
        ir.contains("@scoop.core.substring("),
        "single-file LLVM 路径应把可编译 sysroot 源中的 substring helper 编进当前模块"
    );
}

#[test]
fn box_int_to_any_uses_addrspace_1_ref_pointer() {
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
fn sync_mutex_runtime_calls_use_addrspace_1_object_pointers() {
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
fn string_literal_uses_addrspace_1_gc_string_object() {
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
fn object_member_call_uses_gc_managed_singleton_receiver() {
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

    assert!(
        ir.contains("@__scoop_object_instance__a.Helper = internal global ptr addrspace(1) null"),
        "object 单例槽应保存 GC-managed receiver 指针"
    );
    assert!(
        ir.contains("@scoop_alloc_typed"),
        "object 单例值应通过 typed alloc 生成真实 Ref 对象"
    );
    assert!(
        ir.contains("call i64 @a.Helper.run(ptr addrspace(1)"),
        "object member call 应把 addrspace(1) receiver 传给成员函数"
    );
    assert!(
        !ir.contains("call i64 @a.Helper.run(ptr @__scoop_object_instance__a.Helper)"),
        "member call 不应再把默认地址空间全局地址直接当 receiver 传递"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "object member call 修复不应退回 addrspacecast 打补丁"
    );
}

#[test]
fn println_int_lowers_via_string_formatting_without_print_int_helpers() {
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
fn array_of_any_uses_ref_element_runtime_apis_without_ptr_to_u64() {
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
        ir.contains("@scoop_array_builder_push_ref"),
        "Array<Any> 的 array literal builder 应走 scoop_array_builder_push_ref"
    );
    assert!(
        ir.contains("@scoop_array_get_ref"),
        "Array<Any>.get 应走 scoop_array_get_ref"
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
fn array_of_string_uses_ref_element_runtime_apis_without_ptr_to_u64() {
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
        ir.contains("@scoop_array_builder_push_ref"),
        "Array<String> 的 array literal builder 应走 scoop_array_builder_push_ref"
    );
    assert!(
        ir.contains("@scoop_array_get_ref"),
        "Array<String>.get 应走 scoop_array_get_ref"
    );
    assert!(
        ir.contains("@scoop_array_set_ref"),
        "MutableArray<String>.set 应走 scoop_array_set_ref"
    );
    assert!(
        !ir.contains("ptr_to_u64"),
        "String 元素路径不应把 GC 指针编码为 u64（ptr_to_u64）"
    );
    assert!(
        !ir.contains("u64_to_string"),
        "String 元素路径不应从 u64 解码回 GC 字符串指针（u64_to_string）"
    );
    assert!(
        !ir.contains("addrspacecast"),
        "String array 路径不应引入 addrspacecast"
    );
}

#[test]
fn enum_single_field_non_scalar_payload_uses_boxed_variant_path() {
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
        ir.contains("scoop.runtime.EnumBoxedPayload__a_Result__Ok"),
        "single-field struct payload 应生成 boxed payload object type"
    );
    assert!(
        ir.contains("scoop.runtime.EnumBoxedPayload__a_Result__Msg"),
        "single-field tuple payload 应生成 boxed payload object type"
    );
    assert!(
        ir.contains("__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Ok"),
        "boxed struct payload 应生成对应的类型描述符"
    );
    assert!(
        ir.contains("__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Msg"),
        "boxed tuple payload 应生成对应的类型描述符"
    );
}

#[test]
fn missing_main_is_reported() {
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
fn minimal_main_obj_written_is_non_empty() {
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
fn minimal_main_obj_omits_stackmap_section_by_default() {
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
fn minimal_main_obj_with_live_gc_roots_still_omits_stackmap_section() {
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
fn default_explicit_mode_omits_statepoint_intrinsics_and_gc_strategy() {
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
fn stackmap_statepoint_smoke_helper_opt_in_reenables_stackmap_pipeline() {
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
fn stackmap_statepoint_smoke_helper_emits_stackmap_section_when_requested() {
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
fn minimal_main_asm_written_is_non_empty() {
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
fn managed_function_emits_explicit_root_frame_descriptor() {
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

    assert!(
        ir.contains("@__scoop_explicit_root_desc__a_keep"),
        "expected managed function descriptor global\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_offsets__a_keep = internal constant [2 x i32]"),
        "keep() 现在会同时发布参数 root 与 return slot root\n{ir}"
    );
    assert!(
        ir.contains(
            "@__scoop_explicit_root_offsets__a_keep = internal constant [2 x i32] [i32 16, i32 24]"
        ),
        "keep() 的显式 root frame 偏移应从 header 后开始并覆盖参数/返回值 home slot\n{ir}"
    );
}

#[test]
fn managed_function_emits_explicit_root_frame_tls_lifecycle_and_slot_clear() {
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
    let keep_ir = function_ir_named(&ir, "@a.keep(");

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
fn zero_slot_managed_function_still_emits_explicit_root_frame_lifecycle() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let label_ir = function_ir_named(&ir, "@a.label(");

    assert!(
        ir.contains("@__scoop_explicit_root_desc__a_label"),
        "expected managed function to publish an explicit root descriptor\n{ir}"
    );
    assert!(
        label_ir.contains("%explicit_root_frame_storage = alloca ptr"),
        "expected managed function to allocate an explicit frame\n{label_ir}"
    );
    assert!(
        label_ir.contains(
            "store ptr @__scoop_explicit_root_desc__a_label, ptr %explicit_root_frame_desc_ptr"
        ),
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
fn managed_function_reloads_direct_gc_local_from_explicit_frame_after_safepoint() {
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
    let keep_ir = function_ir_named(&ir, "@a.keep(");
    let call_idx = keep_ir
        .find("@scoop_gc_collect_safepoint")
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
fn class_ctor_this_local_reloads_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let entry_ir = function_ir_named(&ir, "@a.entry(");
    let call_idx = entry_ir
        .find("@scoop_gc_collect_safepoint")
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
fn higher_order_aggregate_return_reloads_string_receiver_after_gc_sensitive_arg_eval() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let lambda_ir = function_ir_named_any(&ir, &["@\"scoop.lambda$0\"(", "@\"a.main.$lambda0\"("]);
    let alloc_idx = lambda_ir
        .find("@__scoop_type_desc_runtime__ScoopString")
        .expect("expected concat arg string allocation in closure IR");
    let call_idx = lambda_ir[alloc_idx..]
        .find("@scoop_string_concat")
        .map(|idx| alloc_idx + idx)
        .expect("expected runtime concat call in closure IR");
    let reload_window = &lambda_ir[alloc_idx..call_idx];

    assert!(
        reload_window.contains("load ptr addrspace(1), ptr %explicit_root_frame_slot_0"),
        "String.concat receiver should reload from the explicit frame after GC-sensitive arg evaluation\n{reload_window}"
    );
    assert!(
        lambda_ir[call_idx..].contains(
            "@scoop_string_concat(ptr addrspace(1) %pass_mir_load, ptr addrspace(1) %pass_mir_load1)"
        ),
        "runtime concat call should consume the receiver reloaded from explicit frame home slots\n{}",
        &lambda_ir[call_idx..]
    );
}

#[test]
fn class_ctor_factory_keeps_allocated_object_rooted_across_gc_sensitive_arg_eval() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let make_ir = function_ir_named(&ir, "@a.make(");
    let string_alloc_idx = make_ir
        .find("@__scoop_type_desc_runtime__ScoopString")
        .expect("expected ctor arg f-string allocation in make() IR");
    let reload_window = &make_ir[string_alloc_idx..];

    assert!(
        make_ir.contains(
            "store ptr addrspace(1) %rt_alloc_refactor_class, ptr %refactor_class_ctor_obj_root"
        ),
        "factory class ctor should spill the freshly allocated object before any GC-sensitive arg evaluation\n{make_ir}"
    );
    assert!(
        reload_window.contains(
            "class_ctor_obj_before_invoke = load ptr addrspace(1), ptr %explicit_root_frame_slot_"
        ),
        "ctor arg evaluation should reload the allocated object from its explicit-frame-backed root before invoking ctor init\n{reload_window}"
    );
    assert!(
        reload_window.contains(
            "class_ctor_obj_return = load ptr addrspace(1), ptr %explicit_root_frame_slot_"
        ),
        "factory return should reload the allocated object from its explicit-frame-backed root after ctor init\n{reload_window}"
    );
    assert!(
        !reload_window.contains("ptr addrspace(1) %rt_alloc_class, i32 0, i32 1"),
        "ctor path should not keep using the pre-GC raw class allocation SSA after GC-sensitive arg evaluation\n{reload_window}"
    );
}

#[test]
fn deferred_call_arg_reloads_from_explicit_frame_after_later_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_named(&ir, "@a.run(");
    let take_idx = run_ir
        .find("call ptr addrspace(1) @a.take(")
        .expect("expected call to take() in run() IR");
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
}

#[test]
fn aggregate_call_arg_rebuilds_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_named(&ir, "@a.run(");
    let call_idx = run_ir
        .find("@a.take(")
        .expect("expected call to take() in run() IR");
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
        reload_window.contains("@a.take(ptr %pass_mir_call_arg_reload_0_rebuild")
            || reload_window.contains("@a.take(ptr noundef %pass_mir_call_arg_reload_0_rebuild"),
        "aggregate call arg should pass the rebuilt slot instead of the stale original spill\n{reload_window}"
    );
}

#[test]
fn hidden_sret_aggregate_result_rebuilds_from_explicit_frame_slots() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let run_ir = function_ir_named(&ir, "@a.run(");
    let call_idx = run_ir
        .find("@a.bounce(")
        .expect("expected call to bounce() in run() IR");
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
fn boxed_effect_payload_rebuilds_aggregate_from_explicit_frame_after_safepoint() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let go_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_go");
    let box_idx = go_ir
        .find("refactor_outward_payload_reload_frame_reload")
        .expect("expected refactor outward payload reload in go() IR");
    let reload_window_start = box_idx.saturating_sub(1400);
    let reload_window = &go_ir[reload_window_start..std::cmp::min(box_idx + 400, go_ir.len())];

    assert!(
        go_ir.contains("refactor_outward_payload_reload_rebuild = alloca %a.Named")
            && go_ir.contains("refactor_outward_payload_reload_field_insert_0 = insertvalue %a.Named undef")
            && go_ir.contains(
                "refactor_step_payload_insert = insertvalue %scoop.refactor.StepCase__a_go__case0 undef, %a.Named %refactor_outward_payload_reload, 0"
            ),
        "refactor outward payload should rebuild a fresh aggregate before publishing Step payload\n{go_ir}"
    );
    assert!(
        reload_window.contains("refactor_outward_payload_reload_frame_reload = load ptr addrspace(1)")
            && reload_window.contains(
                "refactor_outward_payload_reload_field_insert_0 = insertvalue %a.Named undef, ptr addrspace(1) %refactor_outward_payload_reload_frame_reload, 0"
            ),
        "refactor outward payload rebuild should reload GC leaf fields from explicit frame home slots\n{reload_window}"
    );
}

#[test]
fn never_returning_managed_function_pops_explicit_root_frame_before_unreachable() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let stop_ir = function_ir_named(&ir, "__scoop_refactor_direct_invoke__a_stop");

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
fn nested_raise_try_catch_uses_innermost_handle_dispatch_contract() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("define i32 @main("),
        "nested Raise.raise try/catch fixture should lower through refactor EffectStep main codegen\n{ir}"
    );
}

#[test]
fn effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload() {
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
    val (Some(_), y) = pair
    println(y)
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@fixtures.t5000j1d.explode"),
        "compiled IR should still contain the tuple-arg effect-step body\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_refactor_direct_invoke__fixtures_t5000j1d_explode")
            && ir.contains(
                "%scoop.refactor.Frame__fixtures_t5000j1d_explode = type { %scoop.runtime.ScoopGcObjectHeader, { %fixtures.t5000j1d.MyOpt, i64 }"
            ),
        "tuple-arg effect-step callable 应继续保留 refactor direct entry 与 tuple payload frame layout，而不是回旧 wrapper\n{ir}"
    );
}

#[test]
fn explicit_frame_layout_flattens_indirect_gc_aggregate_params() {
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
    let session = Session::new().unwrap();
    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let ir = module.print_to_string().to_string();

    let frame_ty = context
        .get_struct_type("scoop.runtime.ScoopExplicitRootFrame$a_first")
        .expect("missing explicit frame type for a.first");
    assert_eq!(
        frame_ty.get_field_types().len(),
        3,
        "expected header + tracked aggregate/root leaf slots for Named.name"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_offsets__a_first = internal constant [2 x i32]"),
        "expected indirect aggregate param to publish tracked root slots\n{ir}"
    );
}

#[test]
fn explicit_frame_layout_covers_hidden_sret_call_temps() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@__scoop_explicit_root_desc__a_useIt"),
        "expected descriptor for hidden-sret caller\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_offsets__a_useIt"),
        "expected hidden-sret caller to emit root offsets table\n{ir}"
    );
}

#[test]
fn top_level_immutable_init_emits_explicit_root_frame_descriptor() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@__scoop_explicit_root_desc____scoop_top_level_val_init__a_greeting"),
        "expected top-level immutable initializer to emit a descriptor global\n{ir}"
    );
}

#[test]
fn effect_state_machine_functions_emit_explicit_root_frame_descriptors() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@__scoop_explicit_root_desc____scoop_refactor_direct_invoke__a_go"),
        "effectful callable entry 应发布 direct-invoke descriptor global\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_desc____scoop_refactor_resume__a_go__case0")
            && ir.contains("@__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__a_go__k0"),
        "effectful callable 的 resume/owner-dispatch 入口也应发布 explicit-root descriptors\n{ir}"
    );
}

#[test]
fn refactor_plain_array_string_get_keeps_string_surface_for_println() {
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
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_println"),
        "expected String println path to lower successfully\n{ir}"
    );
}

#[test]
fn materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = Session::new().unwrap();
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
fn production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/list_and_mutable_list_basic.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = Session::new().unwrap();
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
            .any(|fqn| fqn.starts_with("scoop.core.add")),
        "expected list fixture to materialize MutableList.add instance in pass view, actual callables: {pass_fun_fqns:?}"
    );
    assert!(
        pass_fun_fqns
            .iter()
            .any(|fqn| fqn.starts_with("scoop.core.push")),
        "expected list fixture to materialize MutableArray.push instance in pass view, actual callables: {pass_fun_fqns:?}"
    );

    let mut seen_push_builder_append = false;
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
                    if callee_fqn != "scoop.core.__scoop_array_builder_push" {
                        continue;
                    }
                    let array = transport
                        .array
                        .as_ref()
                        .expect("array builder push call 应发布 array transport metadata");
                    assert_eq!(
                        materialized.types.display(array.element_ty).to_string(),
                        "Int",
                        "callable `{}` 的 array builder push transport element type 应具体化为 Int",
                        fun.fqn
                    );
                    seen_push_builder_append = true;
                }
            }
        }
    }
    assert!(
        seen_push_builder_append,
        "expected MutableArray.push body to retain at least one builder append site in pass view"
    );
}

#[test]
fn production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop");
    let source = SourceFile::load(&fixture).unwrap();
    let session = Session::new().unwrap();
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

    let mut seen_uint8_builder_pushes = 0;
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
            if callee_fqn != "scoop.core.__scoop_array_builder_push" {
                continue;
            }
            let array = transport
                .array
                .as_ref()
                .expect("array builder push call should publish array transport metadata");
            if materialized.types.display(array.element_ty).to_string() != "UInt8" {
                continue;
            }
            assert_eq!(
                materialized
                    .types
                    .display(array.element.source_ty)
                    .to_string(),
                "UInt8",
                "main's UInt8 array builder push should keep UInt8 source surface"
            );
            assert!(
                !array.element.requirements.trace,
                "main's UInt8 array builder push should stay on scalar transport path"
            );
            assert!(
                !array.element.requirements.drop,
                "main's UInt8 array builder push should not claim aggregate drop obligations"
            );
            assert!(
                array.element.boxing.is_none(),
                "main's UInt8 array builder push should not publish composite boxing metadata"
            );
            seen_uint8_builder_pushes += 1;
        }
    }

    assert_eq!(
        seen_uint8_builder_pushes, 2,
        "expected the fixture's bytes array to retain two UInt8 builder push sites"
    );
}

fn maybe_function_ir_named<'a>(ir: &'a str, name_fragment: &str) -> Option<&'a str> {
    for chunk in ir.split("\ndefine ").skip(1) {
        let end = chunk.find("\n}").expect("expected end of function body") + 2;
        let function = &chunk[..end];
        let header = function.lines().next().expect("expected function header");
        if header.contains(name_fragment) {
            return Some(function);
        }
    }
    None
}

fn function_ir_named<'a>(ir: &'a str, name_fragment: &str) -> &'a str {
    maybe_function_ir_named(ir, name_fragment)
        .unwrap_or_else(|| panic!("expected function containing {name_fragment}"))
}

fn function_ir_named_any<'a>(ir: &'a str, name_fragments: &[&str]) -> &'a str {
    for fragment in name_fragments {
        if let Some(function) = maybe_function_ir_named(ir, fragment) {
            return function;
        }
    }
    panic!(
        "expected function containing one of {}",
        name_fragments.join(", ")
    )
}

fn object_contains_stackmap_section(obj: &object::File<'_>) -> bool {
    obj.sections().any(|section| {
        section
            .name()
            .ok()
            .is_some_and(|name| name.contains("llvm_stackmaps"))
    })
}

fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected: &str) -> bool {
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

fn mir_fun_contains_fun_value_call(fun: &crate::mir::FunDecl) -> bool {
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

#[test]
fn legacy_compare_harness_removed_from_llvm_test_source() {
    let source = include_str!("tests.rs");

    for needle in [
        ["lower_single_source_", "legacy("].concat(),
        [
            "legacy_",
            "reachability_emits_object_init_helper_dependency_for_hir_top_level_ref",
        ]
        .concat(),
        ["legacy eager-", "HIR lowering"].concat(),
        ["legacy ", "effect boundary"].concat(),
        ["legacy ", "TLS signal"].concat(),
    ] {
        assert!(
            !source.contains(&needle),
            "stale compare-harness wording should be removed from llvm tests: {needle}"
        );
    }
}

#[test]
fn legacy_effect_backend_removed_source_inventory() {
    let sources = [
        include_str!("emit.rs"),
        include_str!("mod.rs"),
        include_str!("codegen/effect/contract.rs"),
        include_str!("codegen/effect/mod.rs"),
        include_str!("codegen/object_init.rs"),
        include_str!("codegen/runtime_abi.rs"),
        include_str!("../effect/mod.rs"),
        include_str!("../effect/state_machine/mod.rs"),
    ];

    for source in sources {
        for needle in [
            "state_machine_bridge",
            "state_machine_emitter",
            "UnifiedHandleLoweringContract",
            "begin_legacy_effect_boundary",
            "finish_legacy_effect_boundary",
            "production_lowered_hir",
            "legacy_eager_hir",
        ] {
            assert!(
                !source.contains(needle),
                "legacy effect backend marker should be absent: {needle}"
            );
        }
    }
}

#[test]
fn single_effect_lowering_path_source_inventory() {
    let effect_backend_source = include_str!("codegen/effect/mod.rs");
    assert!(
        !effect_backend_source.contains("codegen_handle_expr_via_state_machine"),
        "HIR handle state-machine lowering entry should be removed"
    );
    assert!(
        !effect_backend_source.contains("ContinuationResumeReplayContext"),
        "legacy continuation replay shim should be removed"
    );

    let shared_state_machine_source = include_str!("../effect/state_machine/mod.rs");
    assert!(
        !shared_state_machine_source.contains("segments"),
        "legacy segment builder module should be removed from the shared suspend planner"
    );
    assert!(
        !shared_state_machine_source.contains("transform"),
        "legacy transform module should be removed from the shared suspend planner"
    );

    let shared_effect_source = include_str!("../effect/mod.rs");
    assert!(
        !shared_effect_source.contains("step_summary"),
        "legacy step-summary module should no longer be exported"
    );
}
