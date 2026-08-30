//! M9 tests: the `UInt` basic type and the core GC facilities
//! (milestone9 DESIGN.md section 1) — `UInt` resolution, equality and
//! arithmetic, the reference-type constraint of the `pin` / `unpin` /
//! `getGcHandle` / `releaseGcHandle` intrinsics (spec 14.1's
//! `T : ref` before M12 bounds), the `gcCollect` / `gcStats` hooks,
//! and `PinHandle` / `GcHandle` construction and field access.

use super::*;

/// The `(name, type name)` pairs of `main`'s body locals, in
/// allocation order.
fn main_local_types(module: &hir::Module) -> Vec<(String, String)> {
    let main = &module.functions[module.entry];
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main is a user function")
    };
    body.locals
        .iter()
        .map(|(_, local)| (local.name.clone(), hir::type_name(module, local.ty)))
        .collect()
}

/// The type name of one of `main`'s locals.
fn local_ty(module: &hir::Module, name: &str) -> String {
    main_local_types(module)
        .into_iter()
        .find(|(local, _)| local == name)
        .unwrap_or_else(|| panic!("no local `{name}`"))
        .1
}

// --- UInt ---

#[test]
fn uint_resolves_and_gcstats_returns_it() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("u", Some(ty_named("UInt")), call("gcStats", vec![]))],
    )]);
    let module = lower_user_with_gc(file).expect("the UInt program must lower");
    assert_eq!(local_ty(&module, "u"), "UInt");
    // `gcStats` declares the `UInt` return type.
    let gc_stats = module
        .top_level
        .iter()
        .map(|&id| &module.functions[id])
        .find(|f| f.name == "gcStats")
        .expect("gcStats is declared in the GC core file");
    assert_eq!(module.types[gc_stats.return_ty], Type::UInt);
}

#[test]
fn uint_and_int_are_different_types() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("u", Some(ty_named("UInt")), int_lit(1))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("an Int literal is no UInt");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `u` must be of type UInt, found Int"
    );
}

#[test]
fn uint_arithmetic_comparison_and_equality() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", call("gcStats", vec![])),
            val("b", call("gcStats", vec![])),
            val("sum", binary(BinOp::Add, var("a"), var("b"))),
            val("product", binary(BinOp::Mul, var("a"), var("b"))),
            val("less", binary(BinOp::Lt, var("a"), var("b"))),
            val("same", binary(BinOp::Eq, var("a"), var("b"))),
        ],
    )]);
    let module = lower_user_with_gc(file).expect("UInt arithmetic must lower");
    assert_eq!(local_ty(&module, "sum"), "UInt");
    assert_eq!(local_ty(&module, "product"), "UInt");
    assert_eq!(local_ty(&module, "less"), "Boolean");
    assert_eq!(local_ty(&module, "same"), "Boolean");
}

#[test]
fn uint_mixed_arithmetic_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val(
            "x",
            binary(BinOp::Add, call("gcStats", vec![]), int_lit(1)),
        )],
    )]);
    let errors = lower_user_with_gc(file).expect_err("Int and UInt do not mix");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `+` requires Int or UInt operands of the same type, found UInt and Int"
    );
}

#[test]
fn uint_mixed_equality_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val(
            "x",
            binary(BinOp::Eq, call("gcStats", vec![]), int_lit(1)),
        )],
    )]);
    let errors = lower_user_with_gc(file).expect_err("UInt and Int compare unequal");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `==` requires operands of the same type, found UInt and Int"
    );
}

#[test]
fn uint_boxes_into_any_for_print() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call("print", vec![call("gcStats", vec![])]))],
    )]);
    let module = lower_user_with_gc(file).expect("printing a UInt must lower");
    // `print` takes `Any`, so the UInt argument crosses by boxing.
    assert!(
        hir::dump(&module).contains("Box : Any\n"),
        "the UInt argument must be boxed: {}",
        hir::dump(&module)
    );
}

// --- GC intrinsics: the happy path (golden dump) ---

#[test]
fn gc_intrinsics_golden() {
    let file = file(vec![fun(
        "main",
        vec![
            val("s", str_lit("hello")),
            val("h", call("pin", vec![var("s")])),
            val("s2", call("unpin", vec![var("h")])),
            val("g", call("getGcHandle", vec![var("s")])),
            val("s3", call("releaseGcHandle", vec![var("g")])),
            stmt(call("gcCollect", vec![])),
            val_ty("n", Some(ty_named("UInt")), call("gcStats", vec![])),
        ],
    )]);
    let module = lower_user_with_gc(file).expect("the GC program must lower");
    assert_eq!(local_ty(&module, "s2"), "String");
    assert_eq!(local_ty(&module, "s3"), "String");
    assert_eq!(local_ty(&module, "n"), "UInt");
    let expected = "\
Module
  struct PinHandle
    field raw: UInt
  struct GcHandle
    field raw: UInt
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun pin<T>(): PinHandle <intrinsic rt_pin>
  fun unpin<T>(): T0 <intrinsic rt_unpin>
  fun getGcHandle<T>(): GcHandle <intrinsic rt_get_handle>
  fun releaseGcHandle<T>(): T0 <intrinsic rt_release_handle>
  fun gcCollect(): Unit <intrinsic rt_gc_collect>
  fun gcStats(): UInt <intrinsic rt_gc_stats>
  fun main(): Unit
    val local0
      StringLiteral \"hello\" : String
    val local1
      Call pin<String> : PinHandle
        Local s : String
    val local2
      Call unpin<String> : String
        Local h : PinHandle
    val local3
      Call getGcHandle<String> : GcHandle
        Local s : String
    val local4
      Call releaseGcHandle<String> : String
        Local g : GcHandle
    Call gcCollect : Unit
    val local5
      Call gcStats : UInt
  entry main
  instance pin<String>
  instance unpin<String>
  instance getGcHandle<String>
  instance releaseGcHandle<String>
";
    assert_eq!(hir::dump(&module), expected);
}

// --- GC intrinsics: the reference-type constraint ---

#[test]
fn pin_rejects_an_int_argument() {
    let file = file(vec![fun(
        "main",
        vec![val("h", call("pin", vec![int_lit(42)]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("pinning a value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pin requires a reference type argument, found Int"
    );
    // The user file is index 2 (core files are 0 and 1).
    assert_eq!(errors[0].file, 2);
}

#[test]
fn pin_rejects_a_struct_argument() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![val(
                "h",
                call("pin", vec![struct_init("Point", vec![int_lit(1)])]),
            )],
        ),
    ]);
    let errors = lower_user_with_gc(file).expect_err("pinning a struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pin requires a reference type argument, found Point"
    );
}

#[test]
fn get_gc_handle_rejects_a_value_argument() {
    let file = file(vec![fun(
        "main",
        vec![val("g", call("getGcHandle", vec![int_lit(42)]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("a handle of a value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "getGcHandle requires a reference type argument, found Int"
    );
}

#[test]
fn unpin_rejects_a_value_type_parameter() {
    let file = file(vec![
        fun_sig(
            "bad",
            vec![],
            vec![("h", ty_generic("PinHandle", vec![ty_named("Int")]))],
            None,
            vec![stmt(call("unpin", vec![var("h")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("unpin of PinHandle<Int> must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "unpin requires a reference type argument, found Int"
    );
}

#[test]
fn release_gc_handle_rejects_a_value_type_parameter() {
    let file = file(vec![
        fun_sig(
            "bad",
            vec![],
            vec![("h", ty_generic("GcHandle", vec![ty_named("Int")]))],
            None,
            vec![stmt(call("releaseGcHandle", vec![var("h")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("releaseGcHandle of GcHandle<Int> must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "releaseGcHandle requires a reference type argument, found Int"
    );
}

#[test]
fn pin_rejects_an_unconstrained_type_parameter() {
    // Without M12 bounds a type parameter may be instantiated with a
    // value type, so `pin(u)` cannot be proven sound at the
    // definition site.
    let file = file(vec![
        fun_sig(
            "f",
            vec!["U"],
            vec![("u", ty_named("U"))],
            None,
            vec![stmt(call("pin", vec![var("u")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("pinning an unconstrained T must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pin requires a reference type argument, found U"
    );
}

#[test]
fn pin_accepts_class_array_any_and_string_references() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", call("pin", vec![call("Throwable", vec![])])),
            val("b", call("pin", vec![array_lit(vec![int_lit(1)])])),
            val("c", call("pin", vec![str_lit("x")])),
            val("d", call("unpin", vec![call("pin", vec![str_lit("y")])])),
        ],
    )]);
    let module = lower_user_with_gc(file).expect("reference arguments must lower");
    // The nested `unpin(pin("y"))` binds T = String through the
    // PinHandle<String> application produced by the inner call.
    assert_eq!(local_ty(&module, "d"), "String");
}

// --- The handle types carry their (phantom) type argument ---

#[test]
fn pin_result_matches_a_pin_handle_annotation() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "h",
            Some(ty_generic("PinHandle", vec![ty_named("String")])),
            call("pin", vec![str_lit("x")]),
        )],
    )]);
    lower_user_with_gc(file).expect("matching handle types must lower");
}

#[test]
fn pin_handle_type_arguments_are_strict() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "h",
            Some(ty_generic("PinHandle", vec![ty_named("Int")])),
            call("pin", vec![str_lit("x")]),
        )],
    )]);
    let errors = lower_user_with_gc(file)
        .expect_err("PinHandle<Int> and PinHandle<String> are different types");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `h` must be of type PinHandle<Int>, found PinHandle<String>"
    );
}

#[test]
fn bare_handle_type_requires_type_arguments() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![("h", ty_named("PinHandle"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("a bare PinHandle must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "generic struct `PinHandle` requires 1 type argument(s)"
    );
}

// --- PinHandle as a plain struct: construction and field access ---

#[test]
fn pin_handle_construction_and_field_access() {
    let file = file(vec![fun(
        "main",
        vec![
            val("u", call("gcStats", vec![])),
            val("h", struct_init("PinHandle", vec![var("u")])),
            val("r", field(var("h"), "raw")),
            // Field access also works on a `PinHandle<T>` application
            // (the `raw` field is an ordinary struct field).
            val_ty(
                "r2",
                Some(ty_named("UInt")),
                field(call("pin", vec![str_lit("x")]), "raw"),
            ),
        ],
    )]);
    let module = lower_user_with_gc(file).expect("handle construction must lower");
    assert_eq!(local_ty(&module, "h"), "PinHandle");
    assert_eq!(local_ty(&module, "r"), "UInt");
    assert_eq!(local_ty(&module, "r2"), "UInt");
}

#[test]
fn pin_handle_construction_checks_the_raw_field() {
    let file = file(vec![fun(
        "main",
        vec![val("h", struct_init("PinHandle", vec![int_lit(1)]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("an Int `raw` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `raw` of `PinHandle` must be of type UInt, found Int"
    );
}

#[test]
fn gcstats_result_is_uint_not_int() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("n", Some(ty_named("Int")), call("gcStats", vec![]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("gcStats does not return Int");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `n` must be of type Int, found UInt"
    );
}
