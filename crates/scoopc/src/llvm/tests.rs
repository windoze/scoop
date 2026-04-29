#[cfg(test)]
mod clayout_tests {
    use super::*;
    use inkwell::values::InstructionOpcode;

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

        let fun = module
            .get_function("main")
            .expect("missing entry function main");
        let entry = fun
            .get_first_basic_block()
            .expect("function has no entry block");

        let mut found_align: Option<u32> = None;
        let mut inst = entry.get_first_instruction();
        while let Some(i) = inst {
            if i.get_opcode() == InstructionOpcode::Alloca {
                let name = i.get_name().and_then(|n| n.to_str().ok()).unwrap_or("");
                if name == "s" {
                    found_align = Some(i.get_alignment().unwrap());
                    break;
                }
            }
            inst = i.get_next_instruction();
        }

        assert_eq!(
            found_align,
            Some(16),
            "expected local alloca for `s` to have align 16 due to @CLayout(aligned=16)"
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
        let fun = module
            .get_function("main")
            .expect("missing entry function main");

        let mut found: Option<u32> = None;
        for bb in fun.get_basic_blocks() {
            let mut inst = bb.get_first_instruction();
            while let Some(i) = inst {
                if i.get_opcode() == InstructionOpcode::Load {
                    let name = i.get_name().and_then(|n| n.to_str().ok()).unwrap_or("");
                    if name.starts_with("load_field") {
                        found = Some(i.get_alignment().unwrap());
                        break;
                    }
                }
                inst = i.get_next_instruction();
            }
            if found.is_some() {
                break;
            }
        }

        assert_eq!(
            found,
            Some(1),
            "expected field load from @CLayout(packed=1) struct to use align 1"
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
use crate::source::{SourceFile, SourceMap};
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

/// 构造 legacy eager-HIR lowering，供边界回归对比“旧后端猜测路径”和主 frontend/via-MIR 路径。
fn lower_single_source_legacy(session: &Session, source: &SourceFile) -> hir::LoweredHir {
    let mut ast = parse_file(source).unwrap();
    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((source, &ast));
        Index::build(&pairs).unwrap()
    };

    let headers = crate::resolve::check_file_headers(source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers).unwrap();

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    env.extend_from_file(source, &ast, &index).unwrap();

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    crate::typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )
    .unwrap();
    crate::typecheck::check_file_type_layouts(&index, &env, &mut typecheck_types, builtins)
        .unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        unit.push((&file.source, &file.ast));
    }
    unit.push((source, &ast));

    hir::lower_for_compilation_unit_multi_files(
        source,
        &index,
        &unit,
        &[(source, &ast)],
        &[],
        &typecheck_types,
    )
    .unwrap()
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
fn single_file_frontend_reaches_async_task_helper_instances_through_perform_continuations() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async {
        println("before")
        val x: Int = await __task_from_result(41)
        println("after")
        println(x)
        x + 1
    }
    return 0
}
"#,
    );
    let session = Session::new().unwrap();
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
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
        "scoop.core.__task_create::<Int>",
        "scoop.core.__task_from_result::<Int>",
        "scoop.core.__task_step_pending::<Int>",
        "scoop.core.__task_step_ready::<Int>",
        "scoop.core.println::<Int>",
        "scoop.core.println::<String>",
    ] {
        assert!(
            lowered_fun_fqns.contains(&fqn),
            "single-file frontend 应保留 async 闭包 continuation 所需实例 `{fqn}`，实际函数集合为: {lowered_fun_fqns:?}"
        );
    }
}

#[test]
fn via_mir_direct_class_call_is_not_reinterpreted_as_vtable_dispatch() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000gr_class_dispatch.scoop",
        r#"
package fixtures.t5000gr

import scoop.core.*

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

class Derived() : Base() {
    override fun ping(): Int {
        return 2
    }
}

fun main(): Int {
    val d: Derived = Derived()
    return d.ping()
}
"#,
    );

    let context = Context::create();
    let frontend_ir =
        build_minimal_main_module_with_opt_level(&session, &source, &context, OptLevel::O2)
            .unwrap()
            .print_to_string()
            .to_string();
    assert!(
        !frontend_ir.contains("call_vtable"),
        "via-MIR frontend 已把 exact receiver class call 去虚化为 direct call，backend 不应再按 FQN 猜回 vtable dispatch:\n{frontend_ir}"
    );

    let legacy_lowered = lower_single_source_legacy(&session, &source);
    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let legacy_ir =
        emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &legacy_lowered)
            .unwrap();
    assert!(
        legacy_ir.contains("call_vtable"),
        "legacy eager-HIR lowering 仍保留 dispatch_call_sites 时，backend 应只按 side table 走 vtable dispatch，而不是因为 class member FQN 自动去虚化:\n{legacy_ir}"
    );
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
        ir.contains("store float 1.500000e+00, ptr %absorbed"),
        "Unsuffixed Float literals in Float32 contexts should lower as LLVM float constants"
    );
    assert!(
        ir.contains("fcmp olt double"),
        "Float comparisons should use ordered LLVM floating-point predicates"
    );
    assert!(
        ir.contains("fcmp oeq float"),
        "Float equality should use ordered equality for NaN-sensitive semantics"
    );
    assert!(
        ir.contains("fcmp une float"),
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
    let lowered = hir::lower_for_compilation_unit_multi_files(
        &source,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &typecheck_types,
    )
    .unwrap();
    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

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
fn lowered_hir_codegen_accepts_multi_file_source_map() {
    let session = Session::new().unwrap();

    let src_lib = SourceFile::new_virtual(
        "<lib>",
        r#"
package fixtures.t0150b

import scoop.core.*

fun helper(x: Int): Int { return x + 1 }
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<main>",
        r#"
package fixtures.t0150b

import scoop.core.*

fun main(): Int { return helper(41) }
"#,
    );

    let mut ast_lib = parse_file(&src_lib).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((&src_lib, &ast_lib));
        pairs.push((&src_main, &ast_main));
        Index::build(&pairs).unwrap()
    };

    let headers_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
    crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &headers_lib).unwrap();

    let headers_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        unit.push((&file.source, &file.ast));
    }
    unit.push((&src_lib, &ast_lib));
    unit.push((&src_main, &ast_main));

    let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
    let typecheck_types = TypeStore::new();
    let lowered = hir::lower_for_compilation_unit_multi_files(
        &src_main,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &typecheck_types,
    )
    .unwrap();

    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }
    let _ = source_map.add_source_clone(&src_lib);
    let entry_source_id = source_map.add_source_clone(&src_main);

    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

    assert!(ir.contains("define i32 @main("));
    assert!(
        ir.contains("@fixtures.t0150b.helper"),
        "expected reachable helper from non-entry file to be present in IR"
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

    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();
    assert!(
        ir.contains("@scoop_println"),
        "materialized generic sysroot direct-call should still route through builtin print lowering"
    );
}

#[test]
fn frontend_codegen_consumes_materialized_generic_direct_call_instances() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package fixtures.t5000e3dr

import scoop.core.*

fun <T> id(x: T): T { return x }

object Box {
    fun <T> memberId(x: T): T { return id(x) }
}

fun main(): Int {
    val a: Int = id(1)
    val b: Int = Box.memberId(2)
    return a + b
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let main = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000e3dr.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    let direct_call_targets = body
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            hir::StmtKind::Val(val) => val.init.as_ref(),
            _ => None,
        })
        .filter_map(|expr| match &expr.kind {
            hir::ExprKind::Call { callee, .. } => match &callee.kind {
                hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    for expected in [
        "fixtures.t5000e3dr.id::<Int>",
        "fixtures.t5000e3dr.Box.memberId::<Int>",
    ] {
        assert!(
            direct_call_targets.contains(&expected),
            "via-MIR frontend lowering 应已把 generic direct-call target 物化为实例 FQN；缺少 `{expected}`，实际为 {direct_call_targets:?}"
        );
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    for expected in [
        "fixtures.t5000e3dr.id::<Int>",
        "fixtures.t5000e3dr.Box.memberId::<Int>",
    ] {
        assert!(
            ir.contains(expected),
            "LLVM IR 应继续直接消费实例身份 `{expected}`，实际 IR:\n{ir}"
        );
    }
    assert!(
        !ir.contains("@fixtures.t5000e3dr.id("),
        "LLVM IR 不应回退到 template target `fixtures.t5000e3dr.id`，实际 IR:\n{ir}"
    );
    assert!(
        !ir.contains("@fixtures.t5000e3dr.Box.memberId("),
        "LLVM IR 不应回退到 template target `fixtures.t5000e3dr.Box.memberId`，实际 IR:\n{ir}"
    );
}

#[test]
fn frontend_codegen_consumes_operator_overload_direct_calls_without_eager_member_inclusion() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1a_operator_direct_call.scoop",
        r#"
package fixtures.t5000j1a

import scoop.core.*

struct Mask(val bits: Int) {
    fun inv(): Mask {
        return Mask(~this.bits)
    }

    fun plus(other: Mask): Mask {
        return Mask(this.bits + other.bits)
    }

    fun shl(bits: Int): Mask {
        return Mask(this.bits << bits)
    }

    fun minus(other: Mask): Mask {
        return Mask(this.bits - other.bits)
    }
}

fun main(): Int {
    val a: Mask = Mask(3)
    val b: Mask = Mask(4)
    val c: Mask = ~a
    val d: Mask = a + b
    val e: Mask = a << 2
    return c.bits + d.bits + e.bits
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
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000j1a.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    let direct_call_targets = body
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            hir::StmtKind::Val(val) => val.init.as_ref(),
            _ => None,
        })
        .filter_map(|expr| match &expr.kind {
            hir::ExprKind::Call { callee, .. } => match &callee.kind {
                hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    for expected in [
        "fixtures.t5000j1a.Mask.inv",
        "fixtures.t5000j1a.Mask.plus",
        "fixtures.t5000j1a.Mask.shl",
    ] {
        assert!(
            direct_call_targets.contains(&expected),
            "operator overload site 应在 typed HIR 中改写成 direct-call target `{expected}`，实际为 {direct_call_targets:?}"
        );
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    for expected in [
        "@fixtures.t5000j1a.Mask.inv(",
        "@fixtures.t5000j1a.Mask.plus(",
        "@fixtures.t5000j1a.Mask.shl(",
    ] {
        assert!(
            ir.contains(expected),
            "production LLVM 应继续消费 operator direct-call target `{expected}`，实际 IR:\n{ir}"
        );
    }
    assert!(
        !ir.contains("@fixtures.t5000j1a.Mask.minus("),
        "reachability 不应再因 operator overload 兜底把未使用的 `Mask.minus` 带进 IR：\n{ir}"
    );
}

#[test]
fn frontend_codegen_consumes_compare_to_direct_calls_without_eager_member_inclusion() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered main body"))
    }

    fn assert_compare_to_binary(expr: &hir::Expr, op: ast::BinaryOp, expected_fqn: &str) {
        let hir::ExprKind::Binary {
            lhs,
            op: actual_op,
            rhs,
            ..
        } = &expr.kind
        else {
            panic!("compareTo site 应降成二元比较，实际为 {:?}", expr.kind);
        };
        assert_eq!(*actual_op, op);

        let hir::ExprKind::Call { callee, args } = &lhs.kind else {
            panic!(
                "compareTo 比较的左侧应为显式 direct-call，实际为 {:?}",
                lhs.kind
            );
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!(
                "compareTo direct-call 应指向顶层 target，实际为 {:?}",
                callee.kind
            );
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(
            args.len(),
            2,
            "compareTo direct-call 应携带隐式 receiver + rhs"
        );
        assert!(
            matches!(
                rhs.kind,
                hir::ExprKind::Literal(hir::LiteralKind::SynthInt(0))
            ),
            "compareTo 比较右侧应为合成的 `0` 常量，实际为 {:?}",
            rhs.kind
        );
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1b_compare_to_direct_call.scoop",
        r#"
package fixtures.t5000j1b

import scoop.core.*

struct Metric(val score: Int) {
    fun compareTo(other: Metric): Int {
        return this.score - other.score
    }
}

struct Unused(val score: Int) {
    fun compareTo(other: Unused): Int {
        return this.score - other.score
    }
}

fun main(): Int {
    val lhs: Metric = Metric(1)
    val rhs: Metric = Metric(2)
    val lt: Bool = lhs < rhs
    val ge: Bool = lhs >= rhs
    val result: Int = if (lt && !ge) {
        0
    } else {
        1
    }
    return result
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
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000j1b.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    assert_compare_to_binary(
        find_local_init(body, "lt"),
        ast::BinaryOp::Lt,
        "fixtures.t5000j1b.Metric.compareTo",
    );
    assert_compare_to_binary(
        find_local_init(body, "ge"),
        ast::BinaryOp::Ge,
        "fixtures.t5000j1b.Metric.compareTo",
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    assert!(
        ir.contains("@fixtures.t5000j1b.Metric.compareTo("),
        "production LLVM 应继续通过 direct-call reachability 发射 compareTo target，实际 IR:\n{ir}"
    );
    assert!(
        !ir.contains("@fixtures.t5000j1b.Unused.compareTo("),
        "未使用的 compareTo 不应再因 eager inclusion 混入 IR：\n{ir}"
    );
}

#[test]
fn production_codegen_entry_rejects_lowered_hir_without_materialized_pass_view() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h0c_missing_pass_view.scoop",
        r#"
package fixtures.t5000h0c

fun main(): Int {
    return 0
}
"#,
    );

    let legacy_lowered = lower_single_source_legacy(&session, &source);
    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let err = emit_minimal_main_ir_from_production_lowered_hir(
        &source_map,
        entry_source_id,
        &legacy_lowered,
    )
    .expect_err(
        "production codegen 入口不应静默接受缺少 materialized pass view 的 legacy lowering",
    );

    assert!(
        matches!(err, LlvmEmitError::MissingMaterializedPassView),
        "应返回结构化错误指出 production codegen 缺少 canonical pass view，实际为: {err:?}"
    );
}

#[test]
fn production_codegen_body_emission_observes_pass_view_body_presence() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h0e_pass_body_presence.scoop",
        r#"
package fixtures.t5000h0e

import scoop.core.*

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

    let mut codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let id_fqn = "fixtures.t5000h0e.id::<Int>";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir_mut()
            .expect("production frontend 应保留 materialized MIR");
        let owner = materialized
            .pass_view()
            .owner_of_callable(id_fqn)
            .expect("pass view 应能反查 id 实例归属")
            .clone();
        let mut summary = materialized
            .pass_view()
            .instance(&owner)
            .expect("pass view 应能读取 id family")
            .summary()
            .clone();
        summary.body_known = false;
        materialized
            .pass_artifacts_mut()
            .remove_callable_body(id_fqn);
        materialized
            .pass_artifacts_mut()
            .set_instance_summary(owner, summary);
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();

    assert!(
        !ir.lines()
            .any(|line| line.starts_with("define ") && line.contains(id_fqn)),
        "pass view 已移除 `{id_fqn}` 的 canonical body 后，production codegen 不应继续按 HIR body 发射该函数，实际 IR:\n{ir}"
    );
    assert!(
        !ir.contains(&format!("@{id_fqn}(")),
        "pass view 已移除 `{id_fqn}` 的 canonical body 后，production codegen 不应继续发射或调用该函数；实际 IR:\n{ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_materialized_mir_body_without_pass_override() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000i1p5_raw_mir_body.scoop",
        r#"
package fixtures.t5000i1p5

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

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let wrap_fqn = "fixtures.t5000i1p5.wrap::<Int>";
    let id_fqn = "fixtures.t5000i1p5.id::<Int>";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let wrap_mir = materialized
        .pass_view()
        .callable(wrap_fqn)
        .expect("raw materialized wrap body 应进入 pass view");
    assert!(
        !materialized
            .pass_view()
            .callable_body_is_overridden(wrap_fqn),
        "O0 下 wrap body 应是 raw materialized MIR，而不是 pass override"
    );
    assert!(
        mir_fun_contains_direct_call(wrap_mir, id_fqn),
        "raw materialized wrap MIR 应仍包含 wrap -> id direct call"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let wrap_ir = function_ir_named(&ir, wrap_fqn);
    assert!(
        wrap_ir.contains("pass_mir_call"),
        "未被 pass override 的 materialized body 也应经 MIR bridge 发射；若仍走 HIR 兼容 body，则不会出现 MIR bridge call 名称:\n{wrap_ir}"
    );
    assert!(
        wrap_ir.contains(id_fqn),
        "raw materialized wrap MIR 的 direct call target 应被 production LLVM 直接消费:\n{wrap_ir}"
    );
}

#[test]
fn production_codegen_falls_back_from_raw_mir_body_for_declaration_only_direct_calls() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_decl_only_direct_call_fallback.scoop",
        r#"
package fixtures.t5000j2decl

import scoop.core.*

fun mk(): Array<Int> {
    return [4, 5, 6]
}

fun main(): Int {
    val xs = mk()
    return xs.get(0)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let mk_fqn = "fixtures.t5000j2decl.mk";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let mk_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == mk_fqn)
        .expect("raw non-generic mk body 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_direct_call(mk_mir, "scoop.core.__scoop_array_builder_new"),
        "test setup 需要确认 mk 的 raw MIR 仍包含 declaration-only array builder direct call"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let mk_ir = function_ir_named(&ir, mk_fqn);
    assert!(
        !mk_ir.contains("mir.bb"),
        "包含 declaration-only direct call 的 raw non-generic body 应继续退回 HIR-compatible emission，避免把 sysroot/runtime intrinsic 当普通函数链接；实际 IR:\n{mk_ir}"
    );
    assert!(
        mk_ir.contains("@scoop_array_builder_new"),
        "HIR-compatible fallback 应继续把 array builder lowering 到 runtime intrinsic:\n{mk_ir}"
    );
    assert!(
        !mk_ir.contains("@scoop.core.__scoop_array_builder_new("),
        "fallback 后不应继续把 declaration-only helper 当普通顶层函数调用:\n{mk_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_when_variant_pattern_and_extract() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_when_variant_pattern.scoop",
        r#"
package fixtures.t5000j2a

import scoop.core.*

enum Step {
    Hit(val value: Int),
    Miss,
}

fun pick(step: Option<Step>): Int {
    return when (step) {
        Some(Hit(v)) -> v
        Some(Miss) -> 7
        None -> 0
    }
}

fun main(): Int {
    return pick(Some(Hit(41)))
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let pick_fqn = "fixtures.t5000j2a.pick";
    assert!(
        codegen_unit.lowered.materialized_mir().is_some(),
        "production frontend 应保留 materialized MIR"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let pick_ir = function_ir_named(&ir, pick_fqn);
    assert!(
        pick_ir.contains("mir.bb"),
        "`pick` 应通过 production MIR bridge 发射，而不是退回 HIR 兼容 body:\n{pick_ir}"
    );
    assert!(
        pick_ir.contains("pass_mir_variant_match"),
        "variant pattern match 应在 MIR bridge 内直接 lower 到 LLVM:\n{pick_ir}"
    );
    assert!(
        pick_ir.contains("pass_mir_extract_subject"),
        "variant payload binder 的 PatternExtract 应在 MIR bridge 内直接发射:\n{pick_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_when_is_pattern() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_when_is_pattern.scoop",
        r#"
package fixtures.t5000j2b

import scoop.core.*

open class Base()

class Impl() : Base()

class Other() : Base()

fun classify(x: Any): Int {
    return when (x) {
        is Impl -> 1
        is Other -> 2
        else -> 0
    }
}

fun main(): Int {
    return classify(Impl())
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let classify_fqn = "fixtures.t5000j2b.classify";
    assert!(
        codegen_unit.lowered.materialized_mir().is_some(),
        "production frontend 应保留 materialized MIR"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let classify_ir = function_ir_named(&ir, classify_fqn);
    assert!(
        classify_ir.contains("mir.bb"),
        "`classify` 应通过 production MIR bridge 发射，而不是退回 HIR 兼容 body:\n{classify_ir}"
    );
    assert!(
        classify_ir.contains("isa_obj_nonnull"),
        "`when is Type` 应复用运行期 isa/type-check lowering:\n{classify_ir}"
    );
}

#[test]
fn production_codegen_exposes_summary_for_generic_pattern_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_summary_pattern_family.scoop",
        r#"
package fixtures.t5000j2c

import scoop.core.*

fun <T> has_some(step: Option<T>): Bool {
    return when (step) {
        Some(_) -> true
        None -> false
    }
}

fun main(): Int {
    return if (has_some<Int>(Some(41))) 1 else 0
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let has_some_fqn = "fixtures.t5000j2c.has_some::<Int>";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let pass_view = materialized.pass_view();
    let owner = pass_view
        .owner_of_callable(has_some_fqn)
        .expect("generic pattern callable 应归属某个 canonical instance family");
    let family = pass_view
        .instance(owner)
        .expect("pass view 应能查询 generic pattern family");
    assert_eq!(family.root_fqn(), has_some_fqn);
    assert!(
        family.summary().body_known,
        "generic pattern instance 应在 canonical pass view 上暴露 body-known summary"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let has_some_ir = function_ir_named(&ir, has_some_fqn);
    assert!(
        has_some_ir.contains("mir.bb"),
        "generic pattern body 应通过 production MIR bridge 发射，而不是退回 HIR 兼容 body:\n{has_some_ir}"
    );
    assert!(
        has_some_ir.contains("pass_mir_variant_tag_eq"),
        "generic pattern body 的 canonical summary/body 应对应到 MIR variant-tag lowering:\n{has_some_ir}"
    );
}

#[test]
fn production_codegen_loads_indirect_gc_aggregate_pattern_params_before_matching() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_nested_option_param.scoop",
        r#"
package fixtures.t5000j2d

import scoop.core.*

fun show(x: Option<Option<String> >): String {
    return when (x) {
        Some(inner) -> when (inner) {
            Some(s) -> s
            None -> "inner-none"
        }
        None -> "outer-none"
    }
}

fun main(): Int {
    val result = show(Some(Some("hi")))
    return if (result == "hi") 1 else 0
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let show_fqn = "fixtures.t5000j2d.show";
    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let show_ir = function_ir_named(&ir, show_fqn);
    assert!(
        show_ir.contains("load %scoop.core.Option, ptr %0"),
        "indirect GC aggregate param 应先从 ABI 指针实参 load 成真实 enum 值，再进入 MIR pattern lowering:\n{show_ir}"
    );
    assert!(
        !show_ir.contains("ptrtoint ptr %0 to i64"),
        "MIR bridge 不应把 indirect enum param 指针本身错当成 payload/tag 原始值:\n{show_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_top_level_immutable_init_access() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3a_top_level_immutable_init.scoop",
        r#"
package fixtures.t5000j3a_top

import scoop.core.*

val Broken: Int = Raise.raise(RuntimeError.NullAssertionFailed)

fun helper(): Int / Raise<RuntimeError> {
    return Broken
}

fun main(): Int {
    return try {
        helper()
    } catch (e: RuntimeError) {
        11
    }
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3a_top.helper";
    let broken_fqn = "fixtures.t5000j3a_top.Broken";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("request-root 可达 non-generic helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_top_level_ref(helper_mir, broken_fqn),
        "test setup 需要确认 raw helper MIR 通过 TopLevelRef 访问 top-level immutable init"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "top-level immutable init access 应通过 raw materialized MIR bridge 发射，而不是退回 HIR-compatible body:\n{helper_ir}"
    );
    assert!(
        helper_ir.contains("@__scoop_top_level_val_init__fixtures.t5000j3a_top.Broken"),
        "production MIR bridge 应继续通过 top-level init helper 触发初始化:\n{helper_ir}"
    );
    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "top-level immutable init access 经 production MIR 主线后仍应保持显式 outcome boundary，而不是退回 TLS probing:\n{helper_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_object_value_init_access() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3a_object_value_init.scoop",
        r#"
package fixtures.t5000j3a_obj

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val marker: Int = 1
}

fun helper(): Int / Raise<RuntimeError> {
    val _obj = BoomObject
    return 7
}

fun main(): Int {
    return try {
        helper()
    } catch (e: RuntimeError) {
        11
    }
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3a_obj.helper";
    let object_fqn = "fixtures.t5000j3a_obj.BoomObject";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("request-root 可达 non-generic helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_top_level_ref(helper_mir, object_fqn),
        "test setup 需要确认 raw helper MIR 通过 TopLevelRef 访问 object value init"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "object value init access 应通过 raw materialized MIR bridge 发射，而不是退回 HIR-compatible body:\n{helper_ir}"
    );
    assert!(
        helper_ir.contains("@__scoop_object_init__fixtures.t5000j3a_obj.BoomObject"),
        "production MIR bridge 应继续通过 object init helper 触发初始化:\n{helper_ir}"
    );
    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "object value init access 经 production MIR 主线后仍应保持显式 outcome boundary，而不是退回 TLS probing:\n{helper_ir}"
    );
}

#[test]
fn production_reachability_emits_object_init_helper_dependency_for_raw_mir_top_level_ref() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3ar_object_init_helper_dep.scoop",
        r#"
package fixtures.t5000j3ar_obj

object BoomObject {
    init {
        helper()
    }
}

fun helper() {}

fun entry(): Int {
    val _obj = BoomObject
    return 0
}

fun main(): Int {
    return entry()
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let entry_fqn = "fixtures.t5000j3ar_obj.entry";
    let helper_fqn = "fixtures.t5000j3ar_obj.helper";
    let object_fqn = "fixtures.t5000j3ar_obj.BoomObject";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let entry_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == entry_fqn)
        .expect("request-root 可达 non-generic entry 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_top_level_ref(entry_mir, object_fqn),
        "test setup 需要确认 raw entry MIR 通过 TopLevelRef 访问 object value init"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let entry_ir = function_ir_named(&ir, entry_fqn);
    let object_init_ir = function_ir_named(
        &ir,
        "__scoop_object_init__fixtures.t5000j3ar_obj.BoomObject",
    );

    assert!(
        entry_ir.contains("mir.bb"),
        "raw MIR 入口访问 object value init 时仍应走 production MIR bridge:\n{entry_ir}"
    );
    assert!(
        object_init_ir.contains(helper_fqn),
        "object init body 内部唯一调用的 helper 必须继续出现在 object init lowering 中:\n{object_init_ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.starts_with("define ") && line.contains(helper_fqn)),
        "reachability 必须为仅在 object init body 中使用的 helper 发射定义，而不是只留下声明:\n{ir}"
    );
}

#[test]
fn legacy_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3ar_legacy_object_init_helper_dep.scoop",
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
        "legacy HIR reachability 也必须保留 object init body 对 helper 的调用:\n{object_init_ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.starts_with("define ") && line.contains("a.helper")),
        "仅由 object init body 触达的 helper 仍必须在模块中拥有定义:\n{ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_non_capturing_closure_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_non_capturing_closure.scoop",
        r#"
package fixtures.t5000j3b_non_capture

fun helper(): Int {
    val thunk: () -> Int = { 7 }
    return thunk()
}

fun main(): Int {
    return helper()
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3b_non_capture.helper";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("request-root 可达 non-generic helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_make_closure(helper_mir),
        "test setup 需要确认 helper 的 raw MIR 仍包含 MakeClosure 形状"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "non-capturing closure 的 raw MIR body 现应直接走 production MIR bridge，而不是继续退回 HIR-compatible body:\n{helper_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_immutable_capture_closure_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_immutable_capture_closure.scoop",
        r#"
package fixtures.t5000j3b_capture

fun helper(x: Int): Int {
    val y = 3
    val addY: (Int) -> Int = { z -> z + y }
    return addY(x)
}

fun main(): Int {
    return helper(4)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3b_capture.helper";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("immutable capture helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_make_closure(helper_mir),
        "test setup 需要确认 helper 的 raw MIR 仍包含 MakeClosure 形状"
    );
    let lambda_fqn = mir_fun_first_make_closure_fn_ptr(helper_mir)
        .expect("immutable capture helper 的 raw MIR 应暴露 closure fn_ptr");

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);
    let lambda_ir = function_ir_named(&ir, lambda_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "immutable-capturing closure 的外层 helper 现应直接走 production MIR bridge:\n{helper_ir}"
    );
    assert!(
        lambda_ir.contains("mir.bb"),
        "immutable-capturing closure 的 lambda body（含 TupleGet env 解包）现也应直接走 production MIR bridge:\n{lambda_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_mutable_capture_closure_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_mutable_capture_closure.scoop",
        r#"
package fixtures.t5000j3b_mut_capture

fun helper(): Int {
    var x = 1
    val bump: () -> Int = {
        x = x + 1
        x
    }
    val a = bump()
    val b = bump()
    return a * 10 + b
}

fun main(): Int {
    return helper()
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3b_mut_capture.helper";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("mutable capture helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_capture_box(helper_mir),
        "test setup 需要确认 helper 的 raw MIR 已显式包含 CaptureBox* 形状"
    );
    let lambda_fqn = mir_fun_first_make_closure_fn_ptr(helper_mir)
        .expect("mutable capture helper 的 raw MIR 应暴露 closure fn_ptr");

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);
    let lambda_ir = function_ir_named(&ir, lambda_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "mutable-capturing closure 的外层 helper 现应直接走 production MIR bridge:\n{helper_ir}"
    );
    assert!(
        lambda_ir.contains("mir.bb"),
        "mutable-capturing closure 的 lambda body（含 CaptureBoxGet/Set）现也应直接走 production MIR bridge:\n{lambda_ir}"
    );
}

#[test]
fn production_codegen_lowers_raw_mir_fun_value_call_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_fun_value_call.scoop",
        r#"
package fixtures.t5000j3b_fun_value

fun applyTwice(f: (Int) -> Int / Pure!, x: Int): Int {
    val y = f(x)
    return f(y)
}

fun inc(x: Int): Int {
    return x + 1
}

fun main(): Int {
    val f: (Int) -> Int = inc
    return applyTwice(f, 1)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let apply_fqn = "fixtures.t5000j3b_fun_value.applyTwice";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let apply_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == apply_fqn)
        .expect("opaque higher-order helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_fun_value_call(apply_mir),
        "test setup 需要确认 helper 的 raw MIR 仍保留 CallKind::FunValue"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let apply_ir = function_ir_named(&ir, apply_fqn);

    assert!(
        apply_ir.contains("mir.bb"),
        "opaque higher-order FunValueCall helper 现应直接走 production MIR bridge:\n{apply_ir}"
    );
}

#[test]
fn production_codegen_uses_closure_definition_source_for_cross_file_raw_mir_body() {
    let session = Session::new().unwrap();
    let src_lib = SourceFile::new_virtual(
        "<lib>/t5000j3b_cross_file_closure.scoop",
        r#"
package fixtures.t5000j3b_cross_file

fun helper(): Int {
    // 让 closure 字面量 span 明显晚于 main 文件长度，锁定不能继续借用 caller source。
    // 12345678901234567890123456789012345678901234567890
    val thunk: () -> Int = { 123456789 }
    return thunk()
}
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<main>/t5000j3b_cross_file_main.scoop",
        r#"
package fixtures.t5000j3b_cross_file

fun main(): Int { return helper() }
"#,
    );

    let mut ast_lib = parse_file(&src_lib).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((&src_lib, &ast_lib));
        pairs.push((&src_main, &ast_main));
        Index::build(&pairs).unwrap()
    };

    let headers_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
    crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &headers_lib).unwrap();

    let headers_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    env.extend_from_file(&src_lib, &ast_lib, &index).unwrap();
    env.extend_from_file(&src_main, &ast_main, &index).unwrap();

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    for (source, ast, header) in [
        (&src_lib, &ast_lib, &headers_lib),
        (&src_main, &ast_main, &headers_main),
    ] {
        crate::typecheck::check_file_annotations(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_type_refs(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_exprs(
            source,
            ast,
            &index,
            &header.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
    }
    crate::typecheck::check_file_type_layouts(&index, &env, &mut typecheck_types, builtins)
        .unwrap();

    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        compilation_unit.push((&file.source, &file.ast));
    }
    compilation_unit.push((&src_lib, &ast_lib));
    compilation_unit.push((&src_main, &ast_main));
    let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
    let request_source_paths = vec![src_main.path().to_path_buf()];
    let lowered = hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        &index,
        &compilation_unit,
        &files_to_lower,
        &[],
        Some(&env),
        &typecheck_types,
        hir::MirInstanceCollectionOptions {
            request_source_paths: &request_source_paths,
            request_root_mode: crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
            opt_level: OptLevel::O0,
        },
    )
    .unwrap();

    let helper_fqn = "fixtures.t5000j3b_cross_file.helper";
    let materialized = lowered
        .materialized_mir()
        .expect("production lowering 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("跨文件 helper 应进入 caller-side pass 候选");
    let lambda_fqn = mir_fun_first_make_closure_fn_ptr(helper_mir)
        .expect("helper raw MIR 应暴露 closure fn_ptr");

    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }
    let _ = source_map.add_source_clone(&src_lib);
    let entry_source_id = source_map.add_source_clone(&src_main);

    let ir =
        emit_minimal_main_ir_from_production_lowered_hir(&source_map, entry_source_id, &lowered)
            .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);
    let lambda_ir = function_ir_named(&ir, lambda_fqn);

    assert!(
        helper_ir.contains("mir.bb"),
        "跨文件 helper 的 raw MIR body 现应直接走 production MIR bridge:\n{helper_ir}"
    );
    assert!(
        lambda_ir.contains("123456789"),
        "closure body 应按定义源文件解析字面量，而不是继续借用 caller source:\n{lambda_ir}"
    );
}

#[test]
fn production_codegen_still_falls_back_for_raw_mir_implicit_tail_return_body_after_candidate_widening()
 {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3a_implicit_tail_return_fallback.scoop",
        r#"
package fixtures.t5000j3a_tail

import scoop.core.*

fun keepLooping(i: Int): Bool {
    println("while_cond")
    println(i)
    i < 1
}

fun main(): Int {
    return if (keepLooping(0)) 1 else 0
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let helper_fqn = "fixtures.t5000j3a_tail.keepLooping";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let helper_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == helper_fqn)
        .expect("request-root 可达 non-generic helper 应进入 caller-side pass 候选");
    assert!(
        mir_fun_has_implicit_tail_return(helper_mir),
        "test setup 需要确认 helper 的 raw MIR 仍以 Return(None) 保留尾表达式返回约定"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let helper_ir = function_ir_named(&ir, helper_fqn);

    assert!(
        !helper_ir.contains("mir.bb"),
        "隐式尾表达式返回目前尚未形成稳定 raw MIR return 契约，扩大 candidate 选择面后也应继续退回 HIR-compatible body:\n{helper_ir}"
    );
}

#[test]
fn production_codegen_still_falls_back_for_raw_mir_non_init_non_pattern_body_after_candidate_widening()
 {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3a_non_init_non_pattern_fallback.scoop",
        r#"
package fixtures.t5000j3a_scope

fun add(a: Int, b: Int): Int {
    return a + b
}

fun main(): Int {
    return if (add(1, 2) == 3) 3 else 1
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let add_fqn = "fixtures.t5000j3a_scope.add";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let add_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == add_fqn)
        .expect("request-root 可达 non-generic helper 应进入 caller-side pass 候选");
    assert!(
        !mir_fun_has_pattern(add_mir),
        "test setup 需要确认 add 不属于 pattern 扩张范围"
    );
    assert!(
        !mir_fun_contains_top_level_value_ref(add_mir),
        "test setup 需要确认 add 不属于 top-level/object init 扩张范围"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let add_ir = function_ir_named(&ir, add_fqn);

    assert!(
        !add_ir.contains("mir.bb"),
        "普通 arithmetic helper 不应因为 j3a 的 init candidate 放宽而误切到 raw MIR bridge:\n{add_ir}"
    );
}

#[test]
fn production_reachability_falls_back_for_raw_mir_ctor_call_todo_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3a_ctor_call_fallback.scoop",
        r#"
import scoop.core.*

fun cSuper(x: Int): Int {
    println("C.super_arg")
    return x + 1
}

fun bSuper(y: Int): Int {
    println("B.super_arg")
    return y + 1
}

fun callArg(): Int {
    println("call.arg")
    return 10
}

open class A(val a: Int) {
    val x: Int = @Safe do {
        println("A.prop")
        a
    }

    init {
        println("A.init")
    }
}

open class B(val b: Int) : A(bSuper(b)) {
    val y: Int = @Safe do {
        println("B.prop")
        b
    }

    init {
        println("B.init")
    }
}

class C(val c: Int) : B(cSuper(c)) {
    val z: Int = @Safe do {
        println("C.prop")
        c
    }

    init {
        println("C.init")
    }
}

fun entry(): Int {
    val _x: C = C(callArg())
    return 0
}

fun main(): Int {
    return entry()
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let entry_fqn = "entry";
    let c_super_fqn = "cSuper";
    let b_super_fqn = "bSuper";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let entry_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == entry_fqn)
        .expect("request-root 可达 non-generic entry 应进入 caller-side pass 候选");
    assert!(
        mir_fun_contains_todo(entry_mir),
        "test setup 需要确认 entry 的 raw MIR 仍包含 ctor call lowering pending 的 Todo 形状"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let entry_ir = function_ir_named(&ir, entry_fqn);

    assert!(
        !entry_ir.contains("mir.bb"),
        "包含 ctor-call Todo 形状的 raw entry body 应继续退回 HIR-compatible body，避免遗漏 ctor side-table reachability:\n{entry_ir}"
    );
    assert!(
        ir.contains(&format!("define i64 @{c_super_fqn}("))
            && ir.contains(&format!("define i64 @{b_super_fqn}(")),
        "HIR-compatible reachability fallback 应继续保留 ctor super-arg helper definitions；否则 class init super-arg 求值顺序 fixture 会在链接阶段丢失 `{c_super_fqn}` / `{b_super_fqn}`"
    );
}

#[test]
fn production_codegen_lowers_overridden_pass_mir_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h0e2_pass_mir_body.scoop",
        r#"
package fixtures.t5000h0e2

fun <T> id(x: T): T {
    return x
}

fun replacement(x: Int): Int {
    return x + 10
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    return wrap(1)
}
"#,
    );

    let mut codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let wrap_fqn = "fixtures.t5000h0e2.wrap::<Int>";
    let id_fqn = "fixtures.t5000h0e2.id::<Int>";
    let replacement_fqn = "fixtures.t5000h0e2.replacement";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir_mut()
            .expect("production frontend 应保留 materialized MIR");
        let mut rewritten_wrap = materialized
            .callable_view()
            .callable(wrap_fqn)
            .expect("raw callable view 应能读取 wrap materialized body")
            .clone();
        let body = rewritten_wrap
            .body
            .as_mut()
            .expect("wrap canonical body 应存在");
        let mut rewrote_call_target = false;
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let crate::mir::StatementKind::Assign { value, .. } = &mut stmt.kind else {
                    continue;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    continue;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    continue;
                };
                if callee_fqn == id_fqn {
                    *callee_fqn = replacement_fqn.to_string();
                    rewrote_call_target = true;
                }
            }
        }
        assert!(
            rewrote_call_target,
            "test setup 应把 wrap 的 pass MIR direct-call target 从 id 改为 replacement"
        );
        materialized
            .pass_artifacts_mut()
            .replace_callable_body(rewritten_wrap);
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let wrap_ir = function_ir_named(&ir, wrap_fqn);

    assert!(
        wrap_ir.contains(&format!("@{replacement_fqn}(")),
        "pass-rewritten MIR body 应直接改变 production LLVM body 发射，wrap 应调用 replacement:\n{wrap_ir}"
    );
    assert!(
        !wrap_ir.contains(&format!("@{id_fqn}(")),
        "若 production 仍回退 HIR body，wrap 会继续调用 id；实际 IR:\n{wrap_ir}"
    );
    let _replacement_ir = function_ir_named(&ir, replacement_fqn);
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
fn production_codegen_observes_summary_driven_mir_direct_call_inlining() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h1_summary_driven_direct_inline.scoop",
        r#"
package fixtures.t5000h1

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

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let wrap_fqn = "fixtures.t5000h1.wrap::<Int>";
    let id_fqn = "fixtures.t5000h1.id::<Int>";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir()
            .expect("production frontend 应保留 materialized MIR");
        let raw_wrap = materialized
            .callable_view()
            .callable(wrap_fqn)
            .expect("raw callable view 应保留 wrap body");
        assert!(
            mir_fun_contains_direct_call(raw_wrap, id_fqn),
            "raw materialized MIR 应继续保留原始 direct call，证明 inlining 只写 pass artifacts"
        );
        let pass_wrap = materialized
            .pass_view()
            .callable(wrap_fqn)
            .expect("pass view 应保留 wrap body");
        assert!(
            !mir_fun_contains_direct_call(pass_wrap, id_fqn),
            "summary-driven inlining 应改写 pass-visible wrap body"
        );
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let wrap_ir = function_ir_named(&ir, wrap_fqn);

    assert!(
        !wrap_ir.contains(&format!("@{id_fqn}(")),
        "production LLVM 应消费 summary-driven rewritten MIR body，wrap 不应继续调用 id:\n{wrap_ir}"
    );
}

#[test]
fn production_codegen_observes_caller_side_mir_inlining_for_non_generic_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h2_non_generic_caller_inline.scoop",
        r#"
package fixtures.t5000h2

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun caller(x: Int): Int {
    return wrap<Int>(x)
}

fun stable(x: Int): Int {
    return x + 1
}

fun main(): Int {
    return caller(1) + stable(2)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let caller_fqn = "fixtures.t5000h2.caller";
    let stable_fqn = "fixtures.t5000h2.stable";
    let wrap_fqn = "fixtures.t5000h2.wrap::<Int>";
    let id_fqn = "fixtures.t5000h2.id::<Int>";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir()
            .expect("production frontend 应保留 materialized MIR");
        let pass_view = materialized.pass_view();
        assert!(
            pass_view.owner_of_callable(caller_fqn).is_none(),
            "non-generic caller rewrite 不应通过伪造 instance family 生效"
        );
        let pass_caller = pass_view
            .callable(caller_fqn)
            .expect("caller-side MIR pass 应能发布真实 non-generic caller body");
        assert!(
            !mir_fun_contains_direct_call(pass_caller, wrap_fqn),
            "pass caller body 不应继续调用被内联的 wrap"
        );
        assert!(
            !mir_fun_contains_direct_call(pass_caller, id_fqn),
            "pass caller body 不应继续调用 wrap 内部的 id"
        );
        assert!(
            pass_view.callable(stable_fqn).is_none(),
            "未改写的 non-generic stable body 不应默认进入 pass view"
        );
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let caller_ir = function_ir_named(&ir, caller_fqn);
    assert!(
        !caller_ir.contains(&format!("@{wrap_fqn}(")),
        "production LLVM 应消费 caller-side rewritten MIR body，caller 不应继续调用 wrap:\n{caller_ir}"
    );
    assert!(
        !caller_ir.contains(&format!("@{id_fqn}(")),
        "caller-side rewritten MIR body 经过迭代 inlining 后不应继续调用 id:\n{caller_ir}"
    );
    let _stable_ir = function_ir_named(&ir, stable_fqn);
}

#[test]
fn production_codegen_lowers_pass_visible_known_closure_call_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_pass_known_closure.scoop",
        r#"
package fixtures.t5000j3b_pass

fun apply(f: (Int) -> Int / Pure!, x: Int): Int {
    return f(x)
}

fun caller(x: Int): Int {
    val delta = 1
    return apply({ y -> y + delta }, x)
}

fun main(): Int {
    return caller(1)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let caller_fqn = "fixtures.t5000j3b_pass.caller";
    let apply_fqn = "fixtures.t5000j3b_pass.apply";
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let pass_caller = materialized
        .pass_view()
        .callable(caller_fqn)
        .expect("known closure provenance 应发布 caller 的 pass-visible MIR body");
    assert!(
        mir_fun_contains_closure_call(pass_caller),
        "test setup 需要确认 caller 的 pass-visible MIR body 已包含结构化 ClosureCall"
    );

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let caller_ir = function_ir_named(&ir, caller_fqn);
    assert!(
        caller_ir.contains("mir.bb"),
        "known-closure provenance 生成的 pass-visible body 现应直接走 production MIR bridge:\n{caller_ir}"
    );
    assert!(
        !caller_ir.contains(&format!("@{apply_fqn}(")),
        "caller 的 pass-visible MIR body 不应退回高阶 wrapper apply:\n{caller_ir}"
    );
}

#[test]
fn production_codegen_observes_direct_call_only_provenance_wrapper_flattening() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h3_direct_call_only_provenance.scoop",
        r#"
package fixtures.t5000h3

fun <T> id(x: T): T {
    return x
}

fun <T> apply(f: (T) -> T / Pure!, x: T): T {
    return f(x)
}

fun caller(x: Int): Int {
    return apply<Int>(id<Int>, x)
}

fun main(): Int {
    return caller(1)
}
"#,
    );

    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let caller_fqn = "fixtures.t5000h3.caller";
    let apply_fqn = "fixtures.t5000h3.apply::<Int>";
    let id_fqn = "fixtures.t5000h3.id::<Int>";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir()
            .expect("production frontend 应保留 materialized MIR");
        let pass_caller = materialized
            .pass_view()
            .callable(caller_fqn)
            .expect("DirectCallOnly + provenance 应发布 caller-side rewritten MIR body");
        assert!(
            !mir_fun_contains_direct_call(pass_caller, apply_fqn),
            "caller pass body 不应继续调用高阶 wrapper"
        );
        assert!(
            !mir_fun_contains_direct_call(pass_caller, id_fqn),
            "高阶 wrapper 摊平后应继续消除具体 direct function 的小调用边界"
        );
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let caller_ir = function_ir_named(&ir, caller_fqn);
    assert!(
        !caller_ir.contains(&format!("@{apply_fqn}(")),
        "production LLVM 应消费 provenance-driven caller rewrite，caller 不应继续调用 wrapper:\n{caller_ir}"
    );
    assert!(
        !caller_ir.contains(&format!("@{id_fqn}(")),
        "production LLVM 应继续消费后续 direct-call inlining，caller 不应继续调用 id:\n{caller_ir}"
    );
}

#[test]
fn production_reachability_scans_overridden_non_generic_pass_body() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h2_non_generic_override_reachability.scoop",
        r#"
package fixtures.t5000h2

fun original(x: Int): Int {
    return x
}

fun replacement(x: Int): Int {
    return x + 10
}

fun caller(x: Int): Int {
    return original(x)
}

fun main(): Int {
    return caller(1)
}
"#,
    );

    let mut codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let caller_fqn = "fixtures.t5000h2.caller";
    let original_fqn = "fixtures.t5000h2.original";
    let replacement_fqn = "fixtures.t5000h2.replacement";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir_mut()
            .expect("production frontend 应保留 materialized MIR");
        let mut rewritten_caller = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .find(|fun| fun.fqn == caller_fqn)
            .expect("request-root 可达 non-generic caller 应进入 caller-side pass 候选")
            .clone();
        let body = rewritten_caller
            .body
            .as_mut()
            .expect("caller pass candidate 应保留 body");
        let mut rewrote_call_target = false;
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let crate::mir::StatementKind::Assign { value, .. } = &mut stmt.kind else {
                    continue;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    continue;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    continue;
                };
                if callee_fqn == original_fqn {
                    *callee_fqn = replacement_fqn.to_string();
                    rewrote_call_target = true;
                }
            }
        }
        assert!(
            rewrote_call_target,
            "test setup 应把 caller 的 non-generic pass MIR direct-call target 从 original 改为 replacement"
        );
        materialized
            .pass_artifacts_mut()
            .replace_callable_body(rewritten_caller);
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let caller_ir = function_ir_named(&ir, caller_fqn);
    assert!(
        caller_ir.contains(&format!("@{replacement_fqn}(")),
        "reachability/body emission 应消费 non-generic pass override，caller 应调用 replacement:\n{caller_ir}"
    );
    assert!(
        !caller_ir.contains(&format!("@{original_fqn}(")),
        "若 production 仍扫描旧 HIR body，caller 会继续调用 original；实际 IR:\n{caller_ir}"
    );
    let _replacement_ir = function_ir_named(&ir, replacement_fqn);
}

#[test]
fn production_codegen_suspendability_observes_overridden_pass_summary() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000h0e_pass_summary.scoop",
        r#"
package fixtures.t5000h0e

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun <T> outward(x: T): T / (Ask) {
    val ignored: Int = Ask.ask(41)
    return x
}

fun entry(): Int / (Ask) {
    return outward<Int>(1)
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

    let mut codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let outward_fqn = "fixtures.t5000h0e.outward::<Int>";
    {
        let materialized = codegen_unit
            .lowered
            .materialized_mir_mut()
            .expect("production frontend 应保留 materialized MIR");
        let owner = materialized
            .pass_view()
            .owner_of_callable(outward_fqn)
            .expect("pass view 应能反查 outward 实例归属")
            .clone();
        let mut summary = materialized
            .pass_view()
            .instance(&owner)
            .expect("pass view 应能读取 outward family")
            .summary()
            .clone();
        summary.may_outward_effect = false;
        materialized
            .pass_artifacts_mut()
            .set_instance_summary(owner, summary);
    }

    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();
    let entry_ir = function_ir_named(&ir, "fixtures.t5000h0e.entry");

    assert!(
        !entry_ir.contains("@__scoop_effect_call_wrapper__fixtures.t5000h0e.outward::<Int>"),
        "pass summary 覆盖 `{outward_fqn}` 为 non-outward-effect 后，known suspendability cache 应采用 pass summary 而不是重新分析 HIR body:\n{entry_ir}"
    );
}

#[test]
fn cross_file_class_ctor_literal_codegen_uses_correct_source_with_utf8_comments() {
    let session = Session::new().unwrap();

    let src_lib = SourceFile::new_virtual(
        "<lib>",
        r#"
package fixtures.t4016t5a

import scoop.core.*

// 中文注释：跨文件构造器参数不应把 caller span 绑到这里。
class Box(val value: Int)
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<main>",
        r#"
package fixtures.t4016t5a

import scoop.core.*

fun main(): Int {
    val box: Box = Box(7)
    return box.value
}
"#,
    );

    let mut ast_lib = parse_file(&src_lib).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            pairs.push((&file.source, &file.ast));
        }
        pairs.push((&src_lib, &ast_lib));
        pairs.push((&src_main, &ast_main));
        Index::build(&pairs).unwrap()
    };

    let headers_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
    crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &headers_lib).unwrap();

    let headers_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        unit.push((&file.source, &file.ast));
    }
    unit.push((&src_lib, &ast_lib));
    unit.push((&src_main, &ast_main));

    let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
    let typecheck_types = TypeStore::new();
    let lowered = hir::lower_for_compilation_unit_multi_files(
        &src_main,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &typecheck_types,
    )
    .unwrap();

    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }
    let _ = source_map.add_source_clone(&src_lib);
    let entry_source_id = source_map.add_source_clone(&src_main);

    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

    assert!(ir.contains("define i32 @main("));
}

#[test]
fn effect_runtime_intrinsics_are_emitted_as_symbol_calls() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    __scoop_effect_clear()
    __scoop_effect_slot_write(9, 4, 33)
    __scoop_effect_slot_write2(7, 5, 11, 22)
    __scoop_effect_set_active()

    val active: Int = __scoop_effect_is_active()
    val tag: Int = __scoop_effect_slot_read_op_tag()
    val key: Int = __scoop_effect_slot_read_effect_instance_key()
    val len: Int = __scoop_effect_slot_read_len_words()
    val single: Int = __scoop_effect_slot_read_value()
    val w0: Int = __scoop_effect_slot_read_word(0)
    val w1: Int = __scoop_effect_slot_read_word(1)

    // 让返回值依赖这些调用，避免未来优化/重写时被意外删除。
    active + tag + key + len + single + w0 + w1
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("@scoop_effect_is_active"),
        "IR 应包含对 scoop_effect_is_active 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_set_active"),
        "IR 应包含对 scoop_effect_set_active 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_clear"),
        "IR 应包含对 scoop_effect_clear 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_write_u64_2"),
        "IR 应包含对 scoop_effect_perform_slot_write_u64_2 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_write_u64"),
        "IR 应包含对 scoop_effect_perform_slot_write_u64 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_read_op_tag"),
        "IR 应包含对 scoop_effect_perform_slot_read_op_tag 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_read_effect_instance_key"),
        "IR 应包含对 scoop_effect_perform_slot_read_effect_instance_key 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_read_len_words"),
        "IR 应包含对 scoop_effect_perform_slot_read_len_words 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_read_u64"),
        "IR 应包含对 scoop_effect_perform_slot_read_u64 的引用"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_read_u64_at"),
        "IR 应包含对 scoop_effect_perform_slot_read_u64_at 的引用"
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

    let context = Context::create();
    let _module = build_minimal_main_module(&session, &source, &context).unwrap();

    let effect_ctx = context
        .get_struct_type("scoop.runtime.ScoopEffectCtx")
        .expect("effect codegen 应注册 ScoopEffectCtx");
    assert_eq!(effect_ctx.count_fields(), 1);

    let value_transport = context
        .get_struct_type("scoop.runtime.ScoopValueTransport")
        .expect("effect codegen 应注册 ScoopValueTransport");
    assert_eq!(value_transport.count_fields(), 2);

    let effect_signal = context
        .get_struct_type("scoop.runtime.ScoopEffectSignal")
        .expect("effect codegen 应注册 ScoopEffectSignal");
    assert_eq!(effect_signal.count_fields(), 4);
    assert_eq!(
        effect_signal.get_field_types()[2].into_struct_type(),
        value_transport,
        "EffectSignal.payload 应继续复用共享的 ValueTransport contract"
    );

    let effect_outcome = context
        .get_struct_type("scoop.runtime.ScoopEffectOutcome")
        .expect("effect codegen 应注册 ScoopEffectOutcome");
    assert_eq!(effect_outcome.count_fields(), 4);
    assert_eq!(
        effect_outcome.get_field_types()[2].into_struct_type(),
        value_transport,
        "EffectOutcome.complete 应继续走 ValueTransport contract"
    );
    assert_eq!(
        effect_outcome.get_field_types()[3].into_struct_type(),
        effect_signal,
        "EffectOutcome.propagate 分支应显式承载 EffectSignal"
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

    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

    assert!(
        ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
        "ordinary callee perform should still write through the shared gc-ref transport entrypoint"
    );
    assert!(
        ir.contains("rt_alloc_effect_value_box"),
        "multi-payload perform should box the whole tuple payload instead of dropping extra args"
    );
    assert!(
        ir.contains("effect_value_box_payload"),
        "handler binder lowering should unbox the transported tuple payload before reading multiple binders"
    );
    assert!(
        !ir.contains("call void @scoop_effect_perform_slot_write_u64(i32"),
        "multi-payload perform should not fall back to the single-word slot write ABI"
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

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();
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

    let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
    let ir = emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

    assert!(
        ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
        "state-machine perform should also write through the shared gc-ref transport entrypoint"
    );
    assert!(
        ir.contains("rt_alloc_effect_value_box"),
        "state-machine multi-payload perform should box the tuple transport instead of rejecting 2+ args"
    );
    assert!(
        ir.contains("effect_value_box_payload"),
        "state-machine handler binder lowering should unbox the transported tuple payload before reading multiple binders"
    );
    assert!(
        ir.contains("@scoop_continuation_resume_with"),
        "Continuation.resume lowering should route through the shared payload+answer helper entry"
    );
    assert!(
        !ir.contains("@scoop_continuation_resume_into"),
        "Continuation.resume lowering should no longer stage payload by calling the lower-level answer-only helper directly"
    );
    assert!(
        !ir.contains("call void @scoop_effect_perform_slot_write_u64(i32"),
        "state-machine multi-payload perform should not fall back to the single-word slot write ABI"
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
    let entry_ir = function_ir_named(&ir, "a.entry");

    assert!(
        !entry_ir.contains("@scoop_effect_is_active"),
        "签名 effectful 但 body 不会 outward-effect 的直调用不应再保留 TLS active 分流:\n{entry_ir}"
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
    let entry_ir = function_ir_named(&ir, "a.entry");

    assert!(
        !entry_ir.contains("@scoop_effect_is_active"),
        "未调用的 higher-order effect 参数不应让外层 ordinary 直调用保留 TLS 分流:\n{entry_ir}"
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
    let entry_ir = function_ir_named(&ir, "a.entry");

    assert!(
        !entry_ir.contains("@scoop_effect_is_active"),
        "body 不会 outward-effect 的 closure 调用不应再保留 TLS active 分流:\n{entry_ir}"
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
    let entry_ir = function_ir_named(&ir, "a.entry");
    let wrapper_ir = function_ir_named(&ir, "__scoop_effect_call_wrapper__a.outward");

    assert!(
        entry_ir.contains("@__scoop_effect_call_wrapper__a.outward")
            && entry_ir.contains("@scoop_effect_outcome_publish")
            && !entry_ir.contains("@scoop_effect_is_active"),
        "ordinary direct outward-effect call 应改走显式 wrapper + outcome，而不是 post-call TLS active probing:\n{entry_ir}"
    );
    assert!(
        wrapper_ir.contains("@scoop_effect_handler_stack_swap_top")
            && wrapper_ir.contains("@scoop_effect_outcome_consume_current"),
        "direct-call wrapper 应负责安装 ctx 并把 legacy TLS signal 收口到显式 outcome:\n{wrapper_ir}"
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
    let entry_ir = function_ir_named(&ir, "a.entry");

    assert!(
        entry_ir.contains("@scoop_effect_outcome_consume_current")
            && entry_ir.contains("@scoop_effect_outcome_publish")
            && !entry_ir.contains("@scoop_effect_is_active"),
        "outward-effect closure call 应在 higher-order boundary 上显式 consume/publish outcome，而不是 post-call TLS probing:\n{entry_ir}"
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
    let entry_ir = function_ir_named(&ir, "a.entry");

    assert!(
        entry_ir.contains("@scoop_effect_outcome_consume_current")
            && entry_ir.contains("@scoop_effect_outcome_publish")
            && !entry_ir.contains("@scoop_effect_is_active"),
        "effectful funptr call 应在 boundary 上显式 consume/publish outcome，而不是继续依赖 TLS active probing:\n{entry_ir}"
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "outward-effect vtable call 应改走显式 outcome boundary，而不是 post-call TLS active probing:\n{helper_ir}"
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
    let helper_ir = function_ir_named(&ir, "a.helper");

    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "outward-effect itable call 应改走显式 outcome boundary，而不是 post-call TLS active probing:\n{helper_ir}"
    );
}

#[test]
fn object_value_init_with_real_outward_effect_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val marker: Int = 1
}

fun helper(): Int / Raise<RuntimeError> {
    val _obj = BoomObject
    return 7
}

fun main(): Int {
    return try {
        helper()
    } catch (e: RuntimeError) {
        11
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_named(&ir, "a.helper");

    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "object value init access 应改走显式 outcome boundary，而不是 post-call TLS active probing:\n{helper_ir}"
    );
}

#[test]
fn top_level_immutable_init_with_real_outward_effect_uses_explicit_outcome_boundary() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

val Broken: Int = Raise.raise(RuntimeError.NullAssertionFailed)

fun helper(): Int / Raise<RuntimeError> {
    return Broken
}

fun main(): Int {
    return try {
        helper()
    } catch (e: RuntimeError) {
        11
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_named(&ir, "a.helper");

    assert!(
        helper_ir.contains("@scoop_effect_outcome_consume_current")
            && helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "top-level immutable init access 应改走显式 outcome boundary，而不是 post-call TLS active probing:\n{helper_ir}"
    );
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

    assert!(
        !helper_ir.contains("@scoop_effect_handler_stack_swap_top")
            && !helper_ir.contains("@scoop_effect_outcome_consume_current")
            && !helper_ir.contains("@scoop_effect_outcome_publish")
            && !helper_ir.contains("@scoop_effect_is_active"),
        "ordinary `@Extern` 调用不应再安装任何 effect boundary 或 TLS probing:\n{helper_ir}"
    );
}

#[test]
fn async_task_ir_uses_ordinary_scoop_task_helpers_not_legacy_runtime_abi() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async {
        val t: Task<Int> = async { 41 }
        val x: Int = await t
        x + 1
    }
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("scoop.core.__task_create::<Int>"),
        "async sugar 应落到 ordinary Scoop `__task_create` helper，而不是旧 runtime ABI"
    );
    assert!(
        ir.contains("scoop.core.__task_step_pending::<Int>"),
        "async task body 内的 await 应改写成 ordinary Scoop pending step helper，而不是同步 join"
    );
    assert!(
        ir.contains("scoop.core.__task_step_ready::<Int>"),
        "async task body 正常完成时应构造 ordinary Scoop ready step helper，而不是直接返回普通值"
    );
    assert!(
        !ir.contains("@scoop_task_create")
            && !ir.contains("@scoop_task_poll")
            && !ir.contains("@scoop_task_step_pending")
            && !ir.contains("@scoop_task_step_ready")
            && !ir.contains("@scoop_task_join"),
        "ordinary `__task_*` 路径不应再直接依赖 legacy `scoop_task_*` runtime ABI"
    );
}

#[test]
fn single_file_minimal_ir_supports_handled_async_await() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val resultTask: Task<Int> = async {
        val t: Task<Int> = async { 41 }
        val x: Int = await t
        x + 1
    }

    return handle {
        Async.await(resultTask)
    } with {
        Async.await(taskArg: Task<Int>) -> __task_join(taskArg)
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("scoop.core.__task_create::<Int>"),
        "single-file LLVM 路径应继续看到 ordinary Scoop `__task_create` helper"
    );
    assert!(
        ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
        "handled Async.await(...) 的 perform site 应在最小 IR 路径上保留 effect transport lowering"
    );
    assert!(
        ir.contains("scoop.core.__task_join::<Int>"),
        "外层 handled Async.await(...) 的 arm body 应能在最小 IR 路径上看到 ordinary Scoop `__task_join` helper"
    );
    assert!(
        !ir.contains("@scoop_task_create")
            && !ir.contains("@scoop_task_poll")
            && !ir.contains("@scoop_task_join"),
        "minimal LLVM 路径里的 async / await 主线不应再回退到 legacy task runtime ABI"
    );
}

#[test]
fn production_codegen_emits_async_task_helper_definitions_reached_via_hir_compat_scan() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j2_async_helper_defs.scoop",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val resultTask: Task<Int> = async {
        val t: Task<Int> = async { 41 }
        val x: Int = await t
        x + 1
    }

    return handle {
        Async.await(resultTask)
    } with {
        Async.await(taskArg: Task<Int>) -> __task_join(taskArg)
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();

    assert!(
        ir.lines().any(|line| line.starts_with("define ")
            && line.contains("scoop.core.__task_step_ready::<Int>")),
        "production/codegen reachability 应继续发射 async helper 依赖的 `__task_step_ready::<Int>` 定义，而不是只留下声明:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| line.starts_with("define ")
            && line.contains("scoop.core.__task_step_pending::<Int>")),
        "production/codegen reachability 应继续发射 async helper 依赖的 `__task_step_pending::<Int>` 定义，而不是只留下声明:\n{ir}"
    );
}

#[test]
fn task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async { 41 }
    return when (task.step()) {
        TaskStep.Pending -> 0
        TaskStep.Ready(value) -> value
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("scoop.core.step::<Int>"),
        "Task.step() 应落到 ordinary Scoop `scoop.core.step::<Int>` 定义"
    );
    assert!(
        ir.contains("scoop.core.__task_drive_created::<Int>"),
        "ordinary Scoop 的 `Task.step()` 实现应继续调用 `__task_drive_created::<Int>`"
    );
    assert!(
        !ir.contains("@scoop_task_poll"),
        "Task.step() 不应再直接调用 legacy `scoop_task_poll` runtime ABI"
    );
}

#[test]
fn production_codegen_keeps_task_step_manual_async_helpers_defined() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3b_task_step_manual_helpers.scoop",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val inner: Task<Int> = async {
        println("inner")
        41
    }

    val outer: Task<Int> = async {
        println("outer-before")
        val x: Int = await inner
        println("outer-after")
        println(x)
        x + 1
    }

    val step0: TaskStep<Int> = outer.step()
    when (step0) {
        TaskStep.Pending -> println("step0=pending")
        TaskStep.Ready(value) -> {
            println("step0=ready")
            println(value)
        }
    }

    when (outer.step()) {
        TaskStep.Pending -> println("step1=pending")
        TaskStep.Ready(value) -> {
            println("step1=ready")
            println(value)
        }
    }

    when (outer.step()) {
        TaskStep.Pending -> println("step2=pending")
        TaskStep.Ready(value) -> {
            println("step2=ready")
            println(value)
        }
    }

    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let ir = emit_minimal_main_ir_from_production_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        &codegen_unit.lowered,
    )
    .unwrap();

    assert!(
        ir.lines().any(|line| line.starts_with("define ")
            && line.contains("scoop.core.__task_step_ready::<Int>")),
        "manual Task.step() 驱动路径中的 async helper 依赖必须保留 `__task_step_ready::<Int>` 定义，而不是只留下声明:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| line.starts_with("define ")
            && line.contains("scoop.core.__task_step_pending::<Int>")),
        "manual Task.step() 驱动路径中的 async helper 依赖必须保留 `__task_step_pending::<Int>` 定义，而不是只留下声明:\n{ir}"
    );
}

#[test]
fn task_step_ir_uses_seqcst_atomic_claim_and_trap_without_mutex() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async { 41 }
    return when (task.step()) {
        TaskStep.Pending -> 0
        TaskStep.Ready(value) -> value
    }
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("cmpxchg ptr addrspace(1) %class_field_gep, i64 0, i64 1 seq_cst seq_cst"),
        "Task.step() 的 claim acquire 必须保持 seq_cst cmpxchg，以支撑 cross-thread sequential handoff"
    );
    assert!(
        ir.contains("store atomic i64 0, ptr addrspace(1) %class_field_gep seq_cst"),
        "Task.step() 的 claim release 必须保持 seq_cst store，以发布 Waiting/Completed 状态"
    );
    assert!(
        ir.contains("@scoop_panic"),
        "claim 冲突或 Running 观察的 single-driver 误用应继续通过 scoop.core.panic 降到 fatal trap"
    );
    assert!(
        !ir.contains("@scoop_sync_mutex_create")
            && !ir.contains("@scoop_sync_mutex_lock")
            && !ir.contains("@scoop_sync_mutex_unlock")
            && !ir.contains("@scoop_sync_mutex_destroy"),
        "Task.step() 不应再回退到 per-task mutex 实现"
    );
    assert!(
        !ir.contains("@scoop_thread_yield"),
        "claim 冲突不应回退到自旋 yield；应直接 trap"
    );
}

#[test]
fn thread_join_statepoint_preserves_live_gc_locals() {
    let source = SourceFile::new_virtual(
        "<mem>",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/runtime_gc/task_step_cross_thread_sequential_handoff_gc_stress.scoop"
        )),
    );

    let session = Session::new().unwrap();
    let context = Context::create();
    let module = build_minimal_main_module(&session, &source, &context).unwrap();
    let (target_machine, _target_info) =
        target::host_target_machine_with_opt_level(OptLevel::O0).unwrap();
    run_pass_pipeline(&module, &target_machine, OptLevel::O0).unwrap();
    let ir = module.print_to_string().to_string();

    let join_idx = ir
        .find("@scoop_thread_join")
        .expect("IR 应包含 `scoop_thread_join` 调用");
    let join_window_start = join_idx.saturating_sub(400);
    let join_window_end = std::cmp::min(join_idx + 1400, ir.len());
    let join_window = &ir[join_window_start..join_window_end];

    assert!(
        join_window.contains("%inner"),
        "thread.join statepoint 应保留仍在当前 frame 里的 `inner` root\n{join_window}"
    );
    assert!(
        join_window.contains("%outer"),
        "thread.join statepoint 应保留仍在当前 frame 里的 `outer` root\n{join_window}"
    );
    assert!(
        join_window.contains("%worker"),
        "thread.join statepoint 应保留 `worker` root 并在返回后写回槽位\n{join_window}"
    );
    assert!(
        join_window.matches("gc_root_keepalive_").count() >= 3,
        "thread.join statepoint 应显式 spill 至少三个 GC local keepalive，而不是只保留 receiver 参数\n{join_window}"
    );
    assert!(
        join_window.contains(r#"[ "gc-live"("#),
        "thread.join 调用点应继续走 LLVM statepoint `gc-live` roots 合同\n{join_window}"
    );
    assert!(
        join_window.contains("store ptr addrspace(1) %gc_root_keepalive_"),
        "thread.join 返回后应把 relocated keepalive 写回真实 local root 槽位\n{join_window}"
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
        ir.contains("@scoop_format_i64"),
        "IR 应通过 scoop_format_i64 走最小格式化（避免 codegen 侧 varargs snprintf）"
    );
    assert!(
        ir.contains("@scoop_alloc_typed"),
        "println(Int) 需要分配 GC-managed String，应调用/声明 scoop_alloc_typed"
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

    assert!(matches!(err, LlvmEmitError::MissingEntryMain));
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
fn minimal_main_obj_contains_stackmap_section_and_header_is_parseable() {
    let dir = make_temp_dir("minimal_main_obj_contains_stackmap_section");
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

    let stackmap_section = obj
        .sections()
        .find(|s| s.name().ok().is_some_and(|n| n.contains("llvm_stackmaps")))
        .expect("missing stackmap section (llvm_stackmaps)");
    let section_data = stackmap_section
        .data()
        .expect("failed to read stackmap section data");

    let header = super::stackmap::StackMapHeader::parse(section_data)
        .expect("stackmap header should be parseable");
    assert!(
        header.num_records > 0,
        "expected stackmap section to contain at least one record"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn minimal_main_obj_stackmap_roots_contract_is_verifyable() {
    // GC-FIX Phase A1：
    // - 解析 stackmap records；
    // - 固化“roots locations 是可计算的连续后缀”契约；
    // - 单测层面保证：至少出现一个带 roots 的 record（否则校验形同虚设）。
    let dir = make_temp_dir("minimal_main_obj_stackmap_roots_contract_is_verifyable");
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
    let stackmap_section = obj
        .sections()
        .find(|s| s.name().ok().is_some_and(|n| n.contains("llvm_stackmaps")))
        .expect("missing stackmap section (llvm_stackmaps)");
    let section_data = stackmap_section
        .data()
        .expect("failed to read stackmap section data");

    let section = crate::stackmap::StackMapSection::parse(section_data)
        .expect("stackmap section should be parseable (v3)");

    let cfg = if cfg!(target_arch = "x86_64") {
        crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 7,
            fp_dwarf_reg: Some(6),
        }
    } else if cfg!(target_arch = "aarch64") {
        crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 31,
            fp_dwarf_reg: Some(29),
        }
    } else {
        panic!("unsupported test target_arch for stackmap roots contract");
    };

    section
        .verify_roots_contract(cfg)
        .expect("stackmap roots contract should hold");

    let roots_records = section
        .records
        .iter()
        .filter(|rec| {
            rec.locations.iter().any(|loc| {
                matches!(
                    loc.kind,
                    crate::stackmap::StackMapLocationKind::Direct
                        | crate::stackmap::StackMapLocationKind::Indirect
                ) && loc.size == cfg.pointer_size
                    && (loc.dwarf_reg == cfg.sp_dwarf_reg
                        || cfg.fp_dwarf_reg.is_some_and(|fp| fp == loc.dwarf_reg))
            })
        })
        .count();
    let sample = section
        .records
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, rec)| {
            let locs = rec
                .locations
                .iter()
                .enumerate()
                .map(|(j, loc)| {
                    format!(
                        "loc[{j}] kind={:?} size={} reg={} off={}",
                        loc.kind, loc.size, loc.dwarf_reg, loc.offset
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "record[{i}] patchpoint_id=0x{:x} inst_off=0x{:x} locs=[{locs}]",
                rec.patchpoint_id, rec.instruction_offset
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        roots_records > 0,
        "expected at least one record to contain GC roots locations\n{sample}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn statepoint_pipeline_rewrites_scoop_alloc_typed_callsites() {
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
        ir.contains("llvm.experimental.gc.statepoint"),
        "expected rewrite-statepoints-for-gc to emit gc.statepoint intrinsics"
    );
    assert!(
        ir.contains("scoop_alloc_typed"),
        "expected statepoint pipeline to cover scoop_alloc_typed (alloc safepoint boundary)"
    );
    assert!(
        !ir.contains("llvm.experimental.stackmap"),
        "expected stackmap records to come from statepoints, not manual stackmap probes"
    );
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
        ir.contains("@__scoop_explicit_root_offsets__a_keep = internal constant [1 x i32]"),
        "expected keep to contribute one direct ref root slot\n{ir}"
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
        2,
        "expected header + one leaf ref slot for Named.name"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_offsets__a_first = internal constant [1 x i32]"),
        "expected indirect aggregate param to flatten to one root slot\n{ir}"
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
        ir.contains("@__scoop_explicit_root_desc__scoop_effect_step_"),
        "expected effect step function to emit a descriptor global\n{ir}"
    );
    assert!(
        ir.contains("@__scoop_explicit_root_desc__scoop_effect_dispatch_"),
        "expected effect dispatch function to emit a descriptor global\n{ir}"
    );
}

fn function_ir_named<'a>(ir: &'a str, name_fragment: &str) -> &'a str {
    for chunk in ir.split("\ndefine ").skip(1) {
        let end = chunk.find("\n}").expect("expected end of function body") + 2;
        let function = &chunk[..end];
        let header = function.lines().next().expect("expected function header");
        if header.contains(name_fragment) {
            return function;
        }
    }
    panic!("expected function containing {name_fragment}");
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

fn mir_fun_contains_top_level_ref(fun: &crate::mir::FunDecl, expected: &str) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            let crate::mir::StatementKind::Assign {
                value: crate::mir::Rvalue::TopLevelRef(crate::mir::TopLevelRef { fqn }),
                ..
            } = &stmt.kind
            else {
                return false;
            };
            fqn == expected
        })
    })
}

fn mir_fun_contains_top_level_value_ref(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::TopLevelRef(_),
                    ..
                }
            )
        })
    })
}

fn mir_fun_has_pattern(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::PatternMatch { .. }
                        | crate::mir::Rvalue::PatternExtract { .. },
                    ..
                }
            )
        })
    })
}

fn mir_fun_contains_make_closure(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::MakeClosure { .. },
                    ..
                }
            )
        })
    })
}

fn mir_fun_contains_capture_box(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::CaptureBoxNew { .. }
                        | crate::mir::Rvalue::CaptureBoxGet { .. }
                        | crate::mir::Rvalue::CaptureBoxSet { .. },
                    ..
                }
            )
        })
    })
}

fn mir_fun_first_make_closure_fn_ptr(fun: &crate::mir::FunDecl) -> Option<&str> {
    let body = fun.body.as_ref()?;
    for block in &body.blocks {
        for stmt in &block.stmts {
            let crate::mir::StatementKind::Assign {
                value: crate::mir::Rvalue::MakeClosure { fn_ptr, .. },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            return Some(fn_ptr.as_str());
        }
    }
    None
}

fn mir_fun_contains_closure_call(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Call {
                        kind: crate::mir::CallKind::Closure { .. },
                        ..
                    },
                    ..
                }
            )
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

fn mir_fun_contains_todo(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                stmt.kind,
                crate::mir::StatementKind::Assign {
                    value: crate::mir::Rvalue::Todo(_),
                    ..
                }
            )
        }) || matches!(block.terminator.kind, crate::mir::TerminatorKind::Todo(_))
    })
}

fn mir_fun_has_implicit_tail_return(fun: &crate::mir::FunDecl) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        matches!(
            block.terminator.kind,
            crate::mir::TerminatorKind::Return { value: None }
        )
    })
}
