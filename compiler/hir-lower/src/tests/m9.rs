//! M9 tests: the `UInt` basic type and the core GC facilities
//! (milestone9 DESIGN.md section 1) — `UInt` resolution, equality and
//! arithmetic, the M12 `T : ref` reference-kind constraint of the
//! `pin` / `unpin` / `getGcHandle` / `releaseGcHandle` intrinsics,
//! the `gcCollect` / `gcStats` hooks,
//! and `PinHandle` / `GcHandle` construction and field access.

use super::*;

fn class_with_interface(
    name: &str,
    base: Option<&str>,
    interface: TypeRef,
    methods: Vec<FunctionDecl>,
) -> Decl {
    let mut decl = class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        base.map(|base| (base, Vec::new())),
        Vec::new(),
        methods,
    );
    let Decl::Class(class) = &mut decl else {
        unreachable!()
    };
    class.interfaces.push(interface);
    decl
}

#[test]
fn generic_interfaces_apply_variance_and_instantiate_methods() {
    let producer = generic_interface_decl(
        "Producer",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let sink = generic_interface_decl(
        "Sink",
        vec![(ast::Variance::In, "T")],
        vec![bodyless_method(
            false,
            "put",
            vec![("value", ty_named("T"))],
            None,
        )],
    );
    let animal = class_decl(
        ast::ClassModifier::Open,
        "Animal",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let dog = class_decl(
        ast::ClassModifier::Final,
        "Dog",
        Vec::new(),
        Some(("Animal", Vec::new())),
        Vec::new(),
        Vec::new(),
    );
    let dog_producer = class_with_interface(
        "DogProducer",
        None,
        ty_generic("Producer", vec![ty_named("Dog")]),
        vec![override_method_expr(
            "get",
            Vec::new(),
            Some(ty_named("Dog")),
            call("Dog", Vec::new()),
        )],
    );
    let animal_sink = class_with_interface(
        "AnimalSink",
        None,
        ty_generic("Sink", vec![ty_named("Animal")]),
        vec![method_full(
            true,
            false,
            "put",
            vec![("value", ty_named("Animal"))],
            None,
            FunctionBody::Block(block(Vec::new())),
        )],
    );
    let module = lower_user(file(vec![
        producer,
        sink,
        animal,
        dog,
        dog_producer,
        animal_sink,
        fun_expr(
            "widen",
            Vec::new(),
            vec![("p", ty_generic("Producer", vec![ty_named("Dog")]))],
            Some(ty_generic("Producer", vec![ty_named("Animal")])),
            var("p"),
        ),
        fun_expr(
            "narrow",
            Vec::new(),
            vec![("s", ty_generic("Sink", vec![ty_named("Animal")]))],
            Some(ty_generic("Sink", vec![ty_named("Dog")])),
            var("s"),
        ),
        fun_expr(
            "read",
            Vec::new(),
            vec![("p", ty_generic("Producer", vec![ty_named("Dog")]))],
            Some(ty_named("Dog")),
            method_call(var("p"), "get", Vec::new()),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("valid generic interface applications");

    let applications: Vec<_> = module
        .types
        .iter()
        .filter(|(_, ty)| matches!(ty, hir::Type::Interface(_, args) if !args.is_empty()))
        .collect();
    assert!(applications.len() >= 4);
}

#[test]
fn generic_call_infers_through_a_concrete_interface_implementation() {
    let producer = generic_interface_decl(
        "Producer",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let int_producer = class_with_interface(
        "IntProducer",
        None,
        ty_generic("Producer", vec![ty_named("Int")]),
        vec![override_method_expr(
            "get",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(42),
        )],
    );
    let infer = fun_sig(
        "infer",
        vec!["T"],
        vec![("producer", ty_generic("Producer", vec![ty_named("T")]))],
        Some(ty_named("T")),
        vec![ret(Some(method_call(var("producer"), "get", Vec::new())))],
    );
    let module = lower_user(file(vec![
        producer,
        int_producer,
        infer,
        fun(
            "main",
            vec![stmt(call("infer", vec![call("IntProducer", Vec::new())]))],
        ),
    ]))
    .expect("the implemented interface application should constrain T");

    let infer = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "infer").then_some(id))
        .expect("infer function");
    let generic = module
        .generic_functions
        .iter()
        .find_map(|(id, entity)| (entity.function == infer).then_some(id))
        .expect("infer generic entity");
    let request = module
        .instantiations
        .iter()
        .find_map(|(_, request)| (request.generic == generic).then_some(request))
        .expect("inferred invocation");
    assert_eq!(request.type_args, [module.int]);
}

#[test]
fn invariant_interface_does_not_convert_between_arguments() {
    let invariant = generic_interface_decl(
        "Cell",
        vec![(ast::Variance::Invariant, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let errors = lower_user(file(vec![
        invariant,
        class_decl(
            ast::ClassModifier::Open,
            "Animal",
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Dog",
            Vec::new(),
            Some(("Animal", Vec::new())),
            Vec::new(),
            Vec::new(),
        ),
        fun_expr(
            "bad",
            Vec::new(),
            vec![("c", ty_generic("Cell", vec![ty_named("Dog")]))],
            Some(ty_generic("Cell", vec![ty_named("Animal")])),
            var("c"),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("invariant arguments must not convert");
    assert!(
        errors[0]
            .message
            .contains("body of `bad` must be of type Cell<Animal>, found Cell<Dog>")
    );
}

#[test]
fn interface_variance_positions_are_checked_recursively() {
    let errors = lower_user(file(vec![
        generic_interface_decl(
            "Consumer",
            vec![(ast::Variance::In, "T")],
            vec![bodyless_method(
                false,
                "put",
                vec![("value", ty_named("T"))],
                None,
            )],
        ),
        generic_interface_decl(
            "BadProducer",
            vec![(ast::Variance::Out, "T")],
            vec![bodyless_method(
                false,
                "put",
                vec![("value", ty_named("T"))],
                None,
            )],
        ),
        generic_interface_decl(
            "BadNested",
            vec![(ast::Variance::Out, "T")],
            vec![bodyless_method(
                false,
                "make",
                Vec::new(),
                Some(ty_generic("Consumer", vec![ty_named("T")])),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("invalid variance positions must fail");
    assert_eq!(errors.len(), 2);
    assert!(
        errors[0]
            .message
            .contains("covariant type parameter `T` occurs in contravariant position")
    );
    assert!(
        errors[1]
            .message
            .contains("covariant type parameter `T` occurs in contravariant position")
    );
}

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

// --- generic struct declarations and applications ---

#[test]
fn generic_struct_fields_construct_access_and_instantiate_methods() {
    let file = file(vec![
        generic_struct_decl_full(
            "Pair",
            vec!["A", "B"],
            vec![("first", ty_named("A")), ("second", ty_named("B"))],
            vec![],
            vec![method_expr(
                "getSecond",
                vec![],
                Some(ty_named("B")),
                var("second"),
            )],
        ),
        fun(
            "main",
            vec![
                val(
                    "pair",
                    struct_init("Pair", vec![int_lit(7), str_lit("seven")]),
                ),
                val("first", field(var("pair"), "first")),
                val("second", method_call(var("pair"), "getSecond", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic struct must lower");
    assert_eq!(local_ty(&module, "pair"), "Pair<Int, String>");
    assert_eq!(local_ty(&module, "first"), "Int");
    assert_eq!(local_ty(&module, "second"), "String");
    assert!(
        hir::dump(&module).contains("struct Pair<A, B>\n    field first: T0\n    field second: T1")
    );
}

#[test]
fn nested_generic_struct_fields_participate_in_inference() {
    let file = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        generic_struct_decl(
            "Wrap",
            vec!["T"],
            vec![("box", ty_generic("Box", vec![ty_named("T")]))],
        ),
        fun(
            "main",
            vec![
                val(
                    "wrapped",
                    struct_init("Wrap", vec![struct_init("Box", vec![str_lit("value")])]),
                ),
                val("value", field(field(var("wrapped"), "box"), "value")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("nested generic structs must lower");
    assert_eq!(local_ty(&module, "wrapped"), "Wrap<String>");
    assert_eq!(local_ty(&module, "value"), "String");
}

#[test]
fn duplicate_generic_struct_type_parameter_is_an_error() {
    let file = file(vec![
        generic_struct_decl("Bad", vec!["T", "T"], vec![("value", ty_named("T"))]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate type parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate type parameter `T`");
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
  struct PinHandle<T : ref>
    field raw: UInt
  struct GcHandle<T : ref>
    field raw: UInt
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
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
  fun pin<T : ref>(): PinHandle<T0> <intrinsic rt_pin>
  fun unpin<T : ref>(): T0 <intrinsic rt_unpin>
  fun getGcHandle<T : ref>(): GcHandle<T0> <intrinsic rt_get_handle>
  fun releaseGcHandle<T : ref>(): T0 <intrinsic rt_release_handle>
  fun gcCollect(): Unit <intrinsic rt_gc_collect>
  fun gcStats(): UInt <intrinsic rt_gc_stats>
  fun main(): Unit
    val local0
      StringLiteral \"hello\" : String
    val local1
      Call pin<String> : PinHandle<String>
        Local s : String
    val local2
      Call unpin<String> : String
        Local h : PinHandle<String>
    val local3
      Call getGcHandle<String> : GcHandle<String>
        Local s : String
    val local4
      Call releaseGcHandle<String> : String
        Local g : GcHandle<String>
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
        "type argument `Int` for `T` of function `pin` must satisfy `ref`"
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
        "type argument `Point` for `T` of function `pin` must satisfy `ref`"
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
        "type argument `Int` for `T` of function `getGcHandle` must satisfy `ref`"
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
    assert!(errors.iter().any(|error| error.message
        == "type argument `Int` for `T` of struct `PinHandle` must satisfy `ref`"));
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
    assert!(errors.iter().any(|error| error.message
        == "type argument `Int` for `T` of struct `GcHandle` must satisfy `ref`"));
}

#[test]
fn pin_rejects_an_unconstrained_type_parameter() {
    // An unconstrained parameter may still be instantiated with a value
    // type, so it cannot satisfy the core API's M12 `T : ref` bound.
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
        "type argument `U` for `T` of function `pin` must satisfy `ref`"
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
        "type argument `Int` for `T` of struct `PinHandle` must satisfy `ref`"
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

// --- PinHandle as a generic struct: construction and field access ---

#[test]
fn pin_handle_construction_and_field_access() {
    let file = file(vec![fun(
        "main",
        vec![
            val("u", call("gcStats", vec![])),
            val_ty(
                "h",
                Some(ty_generic("PinHandle", vec![ty_named("String")])),
                struct_init("PinHandle", vec![var("u")]),
            ),
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
    assert_eq!(local_ty(&module, "h"), "PinHandle<String>");
    assert_eq!(local_ty(&module, "r"), "UInt");
    assert_eq!(local_ty(&module, "r2"), "UInt");
}

#[test]
fn pin_handle_construction_checks_the_raw_field() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "h",
            Some(ty_generic("PinHandle", vec![ty_named("String")])),
            struct_init("PinHandle", vec![int_lit(1)]),
        )],
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
