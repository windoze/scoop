//! M6 tests: the reference-type hierarchy — class and interface
//! declarations, inheritance and override rules, interface
//! implementation, method calls (resolved against the receiver's
//! static type), class field reads with base-chain layout, boxing at
//! subtype crossings, `is` / `as` / `as?` / `===`, smart casts, and
//! struct / enum member functions. One negative test per diagnostic;
//! golden dumps and structural assertions lock the output shape.

use super::*;

use ast::ClassModifier::{Abstract, Final, Open};

// --- shared fixtures ---

/// `interface Describable { fun describe(): String }`.
fn describable() -> Decl {
    interface_decl(
        "Describable",
        vec![bodyless_method(
            false,
            "describe",
            vec![],
            Some(ty_named("String")),
        )],
    )
}

/// `open class Shape(val name: String) : Describable { override fun describe(): String = name }`.
fn shape() -> Decl {
    class_decl(
        Open,
        "Shape",
        vec![(false, "name", ty_named("String"))],
        None,
        vec!["Describable"],
        vec![override_method_expr(
            "describe",
            vec![],
            Some(ty_named("String")),
            var("name"),
        )],
    )
}

/// `class Point(val x: Int, var y: Int) : Shape("point") { fun moveTo(nx, ny) }`.
fn point() -> Decl {
    class_decl(
        Final,
        "Point",
        vec![(false, "x", ty_named("Int")), (true, "y", ty_named("Int"))],
        Some(("Shape", vec![str_lit("point")])),
        vec![],
        vec![method(
            "moveTo",
            vec![("nx", ty_named("Int")), ("ny", ty_named("Int"))],
            None,
            vec![stmt(call("println", vec![var("ny")]))],
        )],
    )
}

/// `struct S(val v: Int)`.
fn struct_s() -> Decl {
    struct_decl("S", vec![("v", ty_named("Int"))])
}

fn find_fn(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn body_of<'m>(module: &'m hir::Module, name: &str) -> &'m hir::Body {
    match &module.functions[find_fn(module, name)].kind {
        hir::FunctionKind::User(body) => body,
        other => panic!("function `{name}` must have a user body, found {other:?}"),
    }
}

/// The single `return <value>` expression of a body.
fn returned(body: &hir::Body) -> &hir::Expr {
    match &body.statements.last().expect("a return").kind {
        hir::StatementKind::Return { value: Some(value) } => value,
        other => panic!("expected a return statement, found {other:?}"),
    }
}

fn class_id(module: &hir::Module, name: &str) -> hir::ClassId {
    module
        .classes
        .iter()
        .find(|(_, c)| c.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("class `{name}` must exist"))
}

fn interface_id(module: &hir::Module, name: &str) -> hir::InterfaceId {
    module
        .interfaces
        .iter()
        .find(|(_, i)| i.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("interface `{name}` must exist"))
}

fn interface_ty(module: &hir::Module, name: &str) -> hir::TypeId {
    let id = interface_id(module, name);
    module.interface_applications[module.interfaces[id].self_application].canonical_type
}

fn class_application(module: &hir::Module, id: hir::ClassId) -> hir::ClassApplicationId {
    module.classes[id].self_application
}

// --- positive: golden dump ---

#[test]
fn class_hierarchy_golden() {
    let file = file(vec![describable(), shape(), point(), fun("main", vec![])]);
    let module = lower_user(file).expect("the hierarchy must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException()
  open class Shape(name: String) : Describable
  class Point(x: Int, y: Int)
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Int
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  interface Describable
    fun describe(): String
  fun coreIntEquals(arg1: Int, arg2: Int): Boolean <extern0 abi=scoop symbol=scoop_rt_int_equals>
  fun coreUIntEquals(arg1: UInt, arg2: UInt): Boolean <extern1 abi=scoop symbol=scoop_rt_uint_equals>
  fun coreBooleanEquals(arg1: Boolean, arg2: Boolean): Boolean <extern2 abi=scoop symbol=scoop_rt_bool_equals>
  fun coreStringEquals(arg1: String, arg2: String): Boolean <extern3 abi=scoop symbol=scoop_rt_string_eq>
  fun coreIntToString(arg1: Int): String <extern4 abi=scoop symbol=scoop_rt_int_to_string>
  fun coreUIntToString(arg1: UInt): String <extern5 abi=scoop symbol=scoop_rt_uint_to_string>
  fun coreBooleanToString(arg1: Boolean): String <extern6 abi=scoop symbol=scoop_rt_bool_to_string>
  fun coreIntHash(arg1: Int): Int <extern7 abi=scoop symbol=scoop_rt_int_hash>
  fun coreUIntHash(arg1: UInt): Int <extern8 abi=scoop symbol=scoop_rt_uint_hash>
  fun coreBooleanHash(arg1: Boolean): Int <extern9 abi=scoop symbol=scoop_rt_bool_hash>
  fun coreStringHash(arg1: String): Int <extern10 abi=scoop symbol=scoop_rt_string_hash>
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(arg1: String): Unit <extern11 abi=scoop symbol=scoop_rt_write>
  fun print<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    return
  fun println<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
  entry main
  instance println<Int>
";
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: declaration structure ---

#[test]
fn class_and_interface_structure() {
    let file = file(vec![describable(), shape(), point(), fun("main", vec![])]);
    let module = lower_user(file).expect("the hierarchy must lower");

    let shape_id = class_id(&module, "Shape");
    let point_id = class_id(&module, "Point");
    let describable_id = interface_id(&module, "Describable");

    let shape = &module.classes[shape_id];
    assert_eq!(shape.modifier, hir::ClassModifier::Open);
    assert_eq!(shape.semantic_constructor().len(), 1);
    assert_eq!(shape.interfaces, vec![interface_ty(&module, "Describable")]);

    // Base-class clause with the lowered delegation arguments.
    let point = &module.classes[point_id];
    let (base, args) = point.base_class.as_ref().expect("Point has a base");
    assert!(matches!(module.types[*base], hir::Type::Class(application)
        if module.class_applications[application].template == shape_id
            && module.class_applications[application].arguments.is_empty()));
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0].kind, hir::ExprKind::StringLiteral(_)));

    // Methods carry owner/modality metadata and `this` as parameter 0.
    let describe = &module.functions[find_fn(&module, "Shape.describe")];
    assert_eq!(
        describe.method.map(|method| method.owner),
        Some(module.class_applications[class_application(&module, shape_id)].canonical_type)
    );
    assert_eq!(describe.params[0].name, "this");
    assert_eq!(describe.params.len(), 1); // only `this`

    let iface_method = &module.functions[find_fn(&module, "Describable.describe")];
    assert!(matches!(
        module.types[iface_method.method.expect("a method").owner],
        hir::Type::Interface(application)
            if module.interface_applications[application].template == describable_id
                && module.interface_applications[application].arguments.is_empty()
    ));

    // The interface method identity points directly at its complete function.
    let member = module.interfaces[describable_id].methods[0];
    let sig = &module.functions[module.interface_methods[member].function];
    assert_eq!(sig.name.rsplit('.').next(), Some("describe"));
    assert_eq!(sig.params.len(), 1); // only `this`
    assert_eq!(module.types[sig.return_ty], hir::Type::String);
}

// --- positive: method resolution and field layout ---

#[test]
fn method_calls_resolve_against_the_receiver_type() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_expr(
            "show",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("String")),
            method_call(var("s"), "describe", vec![]),
        ),
        fun_expr(
            "show2",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            method_call(var("d"), "describe", vec![]),
        ),
        // Inherited method: `moveTo` is declared on Point, `describe`
        // is inherited from Shape.
        fun_expr(
            "show3",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("String")),
            method_call(var("p"), "describe", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("method calls must lower");

    let show = returned(body_of(&module, "show"));
    match &show.kind {
        hir::ExprKind::MethodCall { callee, args, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Shape.describe"
            );
            assert!(args.is_empty());
        }
        other => panic!("expected a method call, found {other:?}"),
    }
    let show2 = returned(body_of(&module, "show2"));
    match &show2.kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Describable.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
    // `describe` on a Point receiver resolves to the inherited Shape method.
    let show3 = returned(body_of(&module, "show3"));
    match &show3.kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Shape.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn class_field_layout_is_base_prefix_then_own() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_expr(
            "get_y",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("Int")),
            field(var("p"), "y"),
        ),
        fun_expr(
            "get_name",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("String")),
            field(var("p"), "name"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("field reads must lower");
    let point_id = class_id(&module, "Point");
    let shape_id = class_id(&module, "Shape");

    // Layout: Shape.name = 0, Point.x = 1, Point.y = 2.
    match &returned(body_of(&module, "get_y")).kind {
        hir::ExprKind::FieldAccess { field, .. } => assert_eq!(
            *field,
            hir::FieldRef::ClassField {
                application: class_application(&module, point_id),
                index: 2
            }
        ),
        other => panic!("expected a field access, found {other:?}"),
    }
    match &returned(body_of(&module, "get_name")).kind {
        hir::ExprKind::FieldAccess { field, .. } => assert_eq!(
            *field,
            hir::FieldRef::ClassField {
                application: class_application(&module, shape_id),
                index: 0
            }
        ),
        other => panic!("expected a field access, found {other:?}"),
    }
}

// --- positive: struct / enum methods, `this`, bare member access ---

#[test]
fn struct_methods_and_bare_field_access() {
    let file = file(vec![
        struct_decl_methods(
            "S",
            vec![("v", ty_named("Int"))],
            vec![method_expr("get", vec![], Some(ty_named("Int")), var("v"))],
        ),
        fun_expr(
            "use_it",
            vec![],
            vec![("s", ty_named("S"))],
            Some(ty_named("Int")),
            method_call(var("s"), "get", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("struct methods must lower");

    // `get` is `S.get` with `this: S` as parameter 0; the bare `v` in
    // its body is `this.v`.
    let get = find_fn(&module, "S.get");
    let function = &module.functions[get];
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].name, "this");
    let struct_id = module
        .structs
        .iter()
        .find(|(_, s)| s.name == "S")
        .map(|(id, _)| id)
        .unwrap();
    match &returned(body_of(&module, "S.get")).kind {
        hir::ExprKind::FieldAccess { receiver, field } => {
            assert_eq!(
                *field,
                hir::FieldRef::StructField {
                    application: module.structs[struct_id].self_application,
                    index: 0
                }
            );
            assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
        }
        other => panic!("expected `this.v`, found {other:?}"),
    }

    match &returned(body_of(&module, "use_it")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(module.callable_function(*callee), get);
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn enum_methods_resolve_and_this_is_the_value() {
    let file = file(vec![
        enum_decl_methods(
            "Color",
            vec![],
            vec![variant_unit("Red"), variant_unit("Blue")],
            vec![method_expr(
                "code",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )],
        ),
        fun_expr(
            "f",
            vec![],
            vec![("c", ty_named("Color"))],
            Some(ty_named("Int")),
            method_call(var("c"), "code", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("enum methods must lower");
    let code = &module.functions[find_fn(&module, "Color.code")];
    assert!(matches!(
        module.types[code.method.expect("a method").owner],
        hir::Type::Enum(..)
    ));
    match &returned(body_of(&module, "f")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Color.code"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn bare_method_calls_inside_a_class_mean_this() {
    let file = file(vec![
        describable(),
        shape(),
        class_decl(
            Final,
            "Loud",
            vec![],
            Some(("Shape", vec![str_lit("loud")])),
            vec![],
            vec![method(
                "shout",
                vec![],
                Some(ty_named("String")),
                vec![ret(Some(call("describe", vec![])))],
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("bare method calls must lower");
    match &returned(body_of(&module, "Loud.shout")).kind {
        hir::ExprKind::MethodCall {
            receiver, callee, ..
        } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Shape.describe"
            );
            assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
        }
        other => panic!("expected `this.describe()`, found {other:?}"),
    }
}

// --- positive: boxing ---

#[test]
fn boxing_at_subtype_crossings() {
    let file = file(vec![
        struct_s(),
        fun(
            "main",
            vec![
                // Value type → Any at a `val` annotation: Box.
                val_ty(
                    "a",
                    Some(ty_named("Any")),
                    struct_init("S", vec![int_lit(1)]),
                ),
                // Value type → Any at an argument position: Box.
                stmt(call("take", vec![struct_init("S", vec![int_lit(2)])])),
            ],
        ),
        fun_sig("take", vec![], vec![("x", ty_named("Any"))], None, vec![]),
        // Value type → Any at a return position: Box.
        fun_expr(
            "give",
            vec![],
            vec![],
            Some(ty_named("Any")),
            struct_init("S", vec![int_lit(3)]),
        ),
    ]);
    let module = lower_user(file).expect("boxing must lower");

    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => {
            assert!(matches!(init.kind, hir::ExprKind::Box(_)));
            assert_eq!(module.types[init.ty], hir::Type::Any);
        }
        other => panic!("expected a val decl, found {other:?}"),
    }
    match &main.statements[1].kind {
        hir::StatementKind::Expr(expr) => match &expr.kind {
            hir::ExprKind::Call { args, .. } => {
                assert!(matches!(args[0].kind, hir::ExprKind::Box(_)));
            }
            other => panic!("expected a call, found {other:?}"),
        },
        other => panic!("expected a statement, found {other:?}"),
    }
    assert!(matches!(
        returned(body_of(&module, "give")).kind,
        hir::ExprKind::Box(_)
    ));
}

#[test]
fn class_to_interface_is_a_zero_cost_retype() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "up",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Describable")),
            var("s"),
        ),
        // Array elements adapt too (annotation drives the element type).
        fun_expr(
            "mk",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_generic("Array", vec![ty_named("Describable")])),
            array_lit(vec![var("s")]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("upcasts must lower");

    // No Box: the local is retyped to the interface.
    let up = returned(body_of(&module, "up"));
    assert!(matches!(up.kind, hir::ExprKind::Local(_)));
    assert!(matches!(module.types[up.ty], hir::Type::Interface(..)));

    match &returned(body_of(&module, "mk")).kind {
        hir::ExprKind::ArrayLiteral(elements) => {
            assert!(matches!(elements[0].kind, hir::ExprKind::Local(_)));
            assert!(matches!(
                module.types[elements[0].ty],
                hir::Type::Interface(..)
            ));
        }
        other => panic!("expected an array literal, found {other:?}"),
    }
}

// --- positive: is / as / as? / === ---

#[test]
fn is_cast_and_ref_eq() {
    let file = file(vec![
        describable(),
        shape(),
        struct_s(),
        fun_expr(
            "check",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_named("Boolean")),
            is_ty(var("a"), ty_named("S"), false),
        ),
        // `as` to a value type: check keeps the reference, Unbox
        // extracts the payload.
        fun_expr(
            "down",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_named("S")),
            cast_ty(var("a"), ty_named("S"), false),
        ),
        // `as?`: Option<T>.
        fun_expr(
            "down_opt",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_nullable(ty_named("S"))),
            cast_ty(var("a"), ty_named("S"), true),
        ),
        // `as?` to a reference type.
        fun_expr(
            "side",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_nullable(ty_named("Shape"))),
            cast_ty(var("d"), ty_named("Shape"), true),
        ),
        fun_expr(
            "same",
            vec![],
            vec![("a", ty_named("Any")), ("s", ty_named("Shape"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefEq, var("a"), var("s")),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("type operators must lower");

    match &returned(body_of(&module, "check")).kind {
        hir::ExprKind::IsInstance { check_ty, .. } => {
            assert!(matches!(module.types[*check_ty], hir::Type::Struct(..)));
        }
        other => panic!("expected an is-check, found {other:?}"),
    }
    match &returned(body_of(&module, "down")).kind {
        hir::ExprKind::Unbox(operand) => assert!(matches!(
            operand.kind,
            hir::ExprKind::Cast {
                optional: false,
                ..
            }
        )),
        other => panic!("expected Unbox(Cast), found {other:?}"),
    }
    let down_opt = returned(body_of(&module, "down_opt"));
    match &down_opt.kind {
        hir::ExprKind::Cast { optional: true, .. } => match &module.types[down_opt.ty] {
            hir::Type::Enum(application) => {
                let application = &module.enum_applications[*application];
                assert_eq!(application.template, module.option_enum);
                assert_eq!(application.arguments.len(), 1);
                assert!(matches!(
                    module.types[application.arguments[0]],
                    hir::Type::Struct(..)
                ));
            }
            other => panic!("expected Option<S>, found {other:?}"),
        },
        other => panic!("expected an optional cast, found {other:?}"),
    }
    match &returned(body_of(&module, "side")).kind {
        hir::ExprKind::Cast { optional: true, .. } => {}
        other => panic!("expected an optional cast, found {other:?}"),
    }
    // Reference identity remains the non-overloadable `===` operation.
    assert!(matches!(
        returned(body_of(&module, "same")).kind,
        hir::ExprKind::Binary {
            op: hir::BinOp::RefEq,
            ..
        }
    ));
}

// --- positive: smart casts ---

#[test]
fn smart_cast_narrows_value_types_with_unbox() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("S"), false),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If { then_body, .. } => {
            let value = match &then_body[0].kind {
                hir::StatementKind::Return { value: Some(value) } => value,
                other => panic!("expected a return, found {other:?}"),
            };
            match &value.kind {
                hir::ExprKind::FieldAccess { receiver, field } => {
                    assert!(matches!(field, hir::FieldRef::StructField { index: 0, .. }));
                    // The narrowed access unboxes the Any local.
                    assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
                }
                other => panic!("expected a field access, found {other:?}"),
            }
        }
        other => panic!("expected an if, found {other:?}"),
    }
}

#[test]
fn smart_cast_narrows_class_references_for_free() {
    let file = file(vec![
        describable(),
        shape(),
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("String")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("Shape"), false),
                    vec![ret(Some(method_call(var("x"), "describe", vec![])))],
                    None,
                ),
                ret(Some(str_lit("?"))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If { then_body, .. } => {
            let value = match &then_body[0].kind {
                hir::StatementKind::Return { value: Some(value) } => value,
                other => panic!("expected a return, found {other:?}"),
            };
            match &value.kind {
                hir::ExprKind::MethodCall {
                    receiver, callee, ..
                } => {
                    assert_eq!(
                        module.functions[module.callable_function(*callee)].name,
                        "Shape.describe"
                    );
                    // The receiver is the same local, retyped — no Unbox.
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert!(matches!(module.types[receiver.ty], hir::Type::Class(..)));
                }
                other => panic!("expected a method call, found {other:?}"),
            }
        }
        other => panic!("expected an if, found {other:?}"),
    }
}

#[test]
fn smart_cast_applies_in_negated_else_and_and_rhs() {
    let file = file(vec![
        struct_s(),
        // `if (x !is S) ... else { x: S }`.
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("S"), true),
                    vec![ret(Some(int_lit(0)))],
                    Some(vec![ret(Some(field(var("x"), "v")))]),
                ),
                ret(Some(int_lit(1))),
            ],
        ),
        // `x is S && x.v > 0`: the RHS already sees the narrowing.
        fun_sig(
            "g",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    binary(
                        BinOp::And,
                        is_ty(var("x"), ty_named("S"), false),
                        binary(BinOp::Gt, field(var("x"), "v"), int_lit(0)),
                    ),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If {
            else_body: Some(else_body),
            ..
        } => match &else_body[0].kind {
            hir::StatementKind::Return { value: Some(value) } => match &value.kind {
                hir::ExprKind::FieldAccess { receiver, .. } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
                }
                other => panic!("expected a field access, found {other:?}"),
            },
            other => panic!("expected a return, found {other:?}"),
        },
        other => panic!("expected an if/else, found {other:?}"),
    }

    let g = body_of(&module, "g");
    match &g.statements[0].kind {
        hir::StatementKind::If { cond, .. } => match &cond.kind {
            hir::ExprKind::Binary {
                op: hir::BinOp::And,
                rhs,
                ..
            } => match &rhs.kind {
                hir::ExprKind::Binary { lhs, .. } => match &lhs.kind {
                    hir::ExprKind::FieldAccess { receiver, .. } => {
                        assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
                    }
                    other => panic!("expected a field access, found {other:?}"),
                },
                other => panic!("expected a comparison, found {other:?}"),
            },
            other => panic!("expected a conjunction, found {other:?}"),
        },
        other => panic!("expected an if, found {other:?}"),
    }
}

// --- negative: declarations and inheritance ---

#[test]
fn inheriting_a_final_class_is_an_error() {
    let file = file(vec![
        class_decl(Final, "A", vec![], None, vec![], vec![]),
        class_decl(Final, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("inheriting a final class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `A` is final and cannot be inherited"
    );
}

#[test]
fn base_clause_requires_a_class() {
    let file = file(vec![
        describable(),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("Describable", vec![])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a non-class base must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`Describable` is not a class");
}

#[test]
fn interface_list_requires_interfaces() {
    let file = file(vec![
        class_decl(Final, "A", vec![], None, vec![], vec![]),
        class_decl(Open, "B", vec![], None, vec!["A"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a non-interface in the list must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`A` is not an interface");
}

#[test]
fn duplicate_constructor_property_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int")), (true, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate properties must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate property `x` in class `C`");
}

#[test]
fn duplicate_method_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method("m", vec![], None, vec![]),
                method("m", vec![], None, vec![]),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate methods must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn cyclic_inheritance_is_an_error() {
    let file = file(vec![
        class_decl(Open, "A", vec![], Some(("B", vec![])), vec![], vec![]),
        class_decl(Open, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("cyclic inheritance must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message == "class `A` directly or indirectly inherits from itself"),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn property_shadowing_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![(false, "x", ty_named("Int"))],
            Some(("A", vec![int_lit(1)])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("shadowing properties must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "property `x` of class `B` shadows a property of base class `A`"
    );
}

#[test]
fn base_constructor_arity_is_checked() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![int_lit(1), int_lit(2)])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "constructor of class `A` takes exactly 1 argument, but 2 were supplied"
    );
}

#[test]
fn base_constructor_argument_types_are_checked() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![str_lit("s")])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("wrong argument type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for constructor property `x` of class `A` must be of type Int, found String"
    );
}

// --- negative: override and implementation ---

#[test]
fn open_and_default_open_override_form_one_visible_dispatch_slot() {
    let open_f = with_method_modifier(
        method_expr("f", vec![], Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![open_f]),
        class_decl(
            Open,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            )],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(3),
            )],
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_named("A")), call("C", vec![])),
                stmt(call("println", vec![method_call(var("a"), "f", vec![])])),
            ],
        ),
    ]);
    let output = lower_user_output(file).expect("the override chain must lower without ambiguity");
    let module = &output.export;
    assert_eq!(
        module.functions[find_fn(module, "A.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Open
    );
    assert_eq!(
        module.functions[find_fn(module, "B.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Open
    );
    // The owner class is final, so its otherwise-open override is
    // normalized to an effectively final method in HIR.
    assert_eq!(
        module.functions[find_fn(module, "C.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Final
    );
    let hir::MethodDispatch::Virtual(family) = module.functions[find_fn(module, "A.f")]
        .method
        .expect("method")
        .dispatch
    else {
        panic!("the first open declaration owns a virtual family")
    };
    assert_eq!(
        module.functions[find_fn(module, "B.f")]
            .method
            .expect("method")
            .dispatch,
        hir::MethodDispatch::Virtual(family)
    );
    assert_eq!(
        module.functions[find_fn(module, "C.f")]
            .method
            .expect("method")
            .dispatch,
        hir::MethodDispatch::FinalOverride(family)
    );

    let dispatches = ["A.f", "B.f", "C.f"].map(|name| {
        output
            .local
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == name).then_some(function.method))
            .flatten()
            .expect("the concrete override chain keeps method metadata")
            .dispatch
    });
    let hir::concrete::MethodDispatch::Virtual(local_family) = dispatches[0] else {
        panic!("the concrete base method owns a virtual family")
    };
    assert_eq!(
        dispatches[1],
        hir::concrete::MethodDispatch::Virtual(local_family)
    );
    assert_eq!(
        dispatches[2],
        hir::concrete::MethodDispatch::FinalOverride(local_family)
    );
}

#[test]
fn overriding_a_final_method_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![],
            None,
            vec![],
            vec![method_expr("f", vec![], Some(ty_named("Int")), int_lit(1))],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a final method must not be overridden");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`f` cannot override final method `A.f`");
}

#[test]
fn final_override_closes_the_override_chain() {
    let open_f = with_method_modifier(
        method_expr("f", vec![], Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    let final_override = with_method_modifier(
        override_method_expr("f", vec![], Some(ty_named("Int")), int_lit(2)),
        ast::MethodModifier::Final,
    );
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![open_f]),
        class_decl(
            Open,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![final_override],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(3),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("final override must close the slot");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`f` cannot override final method `B.f`");
}

#[test]
fn fresh_open_method_requires_an_inheritable_class() {
    let open_f = with_method_modifier(method("f", vec![], None, vec![]), ast::MethodModifier::Open);
    let file = file(vec![
        class_decl(Final, "C", vec![], None, vec![], vec![open_f]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a final class cannot introduce an open method");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "open function `f` is only allowed in open or abstract classes"
    );
}

#[test]
fn value_types_cannot_declare_open_methods() {
    let open_f = with_method_modifier(method("f", vec![], None, vec![]), ast::MethodModifier::Open);
    let file = file(vec![
        struct_decl_methods("S", vec![("v", ty_named("Int"))], vec![open_f]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value methods cannot be virtual");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "open function `f` is only allowed in class declarations"
    );
}

#[test]
fn overriding_without_the_modifier_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(
            Open,
            "Shape",
            vec![(false, "name", ty_named("String"))],
            None,
            vec!["Describable"],
            vec![method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                var("name"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a missing `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`describe` overrides `Describable.describe` and must be marked `override`"
    );
}

#[test]
fn override_without_overriding_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![method_full(
                true,
                false,
                "m",
                vec![],
                None,
                FunctionBody::Block(block(vec![])),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a spurious `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}

#[test]
fn unimplemented_interface_method_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(Final, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `C` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn interface_implementation_with_the_wrong_signature_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec!["Describable"],
            vec![method_full(
                true,
                false,
                "describe",
                vec![],
                Some(ty_named("Int")),
                FunctionBody::Expr(Box::new(int_lit(1))),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a signature mismatch must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message
                == "class `C` does not implement interface method `Describable.describe`"),
        "unexpected diagnostics: {errors:?}"
    );
    assert!(
        errors.iter().any(
            |e| e.message == "`describe` is marked `override` but does not override any method"
        ),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn abstract_classes_may_leave_interface_methods_unimplemented() {
    let file = file(vec![
        describable(),
        class_decl(Abstract, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    lower_user(file).expect("abstract classes defer the implementation");
}

// --- negative: abstract and bodies ---

#[test]
fn abstract_class_instantiation_is_an_error() {
    let file = file(vec![
        class_decl(
            Abstract,
            "Base",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "kind", vec![], Some(ty_named("Int")))],
        ),
        fun("main", vec![stmt(call("Base", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("instantiating an abstract class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract class `Base` cannot be instantiated"
    );
}

#[test]
fn class_construction_lowers_to_class_init() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int")), (false, "a", ty_named("Any"))],
            None,
            vec![],
            vec![],
        ),
        fun(
            "main",
            vec![val("c", call("C", vec![int_lit(1), int_lit(2)]))],
        ),
    ]);
    let module = lower_user(file).expect("class construction must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::ClassInit {
                application, args, ..
            } => {
                let class_id = module.class_applications[*application].template;
                assert_eq!(module.classes[class_id].name, "C");
                assert!(matches!(
                    module.types[init.ty],
                    hir::Type::Class(found) if found == *application
                ));
                assert_eq!(args.len(), 2);
                // The Int argument crossing into the `Any` property boxes.
                assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(1)));
                assert!(matches!(args[1].kind, hir::ExprKind::Box(_)));
            }
            other => panic!("expected a ClassInit, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
}

#[test]
fn class_construction_arity_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `C` takes exactly 1 argument, but 0 were supplied"
    );
}

#[test]
fn class_construction_argument_types_are_checked() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![str_lit("s")]))]),
    ]);
    let errors = lower_user(file).expect_err("a wrong argument type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `x` of `C` must be of type Int, found String"
    );
}

#[test]
fn abstract_method_outside_an_abstract_class_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("misplaced abstract must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract function `m` is only allowed in abstract classes"
    );
}

#[test]
fn interface_method_with_a_body_is_an_error() {
    let file = file(vec![
        interface_decl("I", vec![method("m", vec![], None, vec![])]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a bodied interface method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "interface method `I.m` must not have a body"
    );
}

#[test]
fn concrete_method_without_a_body_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(false, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a bodyless concrete method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "function `m` must have a body");
}

#[test]
fn final_generic_member_functions_are_resolved() {
    let mut generic = method_expr(
        "id",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    generic.type_params = vec![type_param("T")];
    let file = file(vec![
        class_decl(Final, "C", vec![], None, vec![], vec![generic]),
        fun(
            "main",
            vec![stmt(method_call(
                call("C", vec![]),
                "id",
                vec![str_lit("ok")],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("a final generic method must lower");
    let method = find_fn(&module, "C.id");
    assert_eq!(module.functions[method].type_param_count(), 1);
    assert_eq!(module.functions[method].type_params()[0].name, "T");
    let hir::FunctionGenericity::GenericMethod {
        definition: generic,
        ..
    } = module.functions[method].genericity
    else {
        panic!("C.id generic entity")
    };
    let (_, request) = module
        .generic_method_applications
        .iter()
        .find(|(_, request)| request.method == generic)
        .expect("the call requests an instance");
    assert_eq!(request.method_arguments.to_vec(), [module.string]);
    assert!(matches!(request.owner, hir::GenericMethodOwner::Class(_)));
}

// --- negative: method calls, fields, assignment ---

#[test]
fn unknown_method_is_an_error() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Int")),
            method_call(var("s"), "nope", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unknown method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Shape` has no method `nope`");
}

#[test]
fn unknown_class_field_is_an_error() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Int")),
            field(var("s"), "zzz"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "class `Shape` has no field `zzz`");
}

#[test]
fn assigning_a_val_property_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![method("m", vec![], None, vec![assign("x", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("assigning a val property must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `x`");
}

#[test]
fn bare_var_property_assignment_in_a_method_stores_through_this() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(true, "y", ty_named("Int"))],
            None,
            vec![],
            vec![method("m", vec![], None, vec![assign("y", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("var property assignment must lower");
    let body = body_of(&module, "C.m");
    match &body.statements[0].kind {
        hir::StatementKind::Assign { target, value } => {
            match target {
                hir::AssignTarget::Field { receiver, field } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert!(matches!(field, hir::FieldRef::ClassField { index: 0, .. }));
                }
                other => panic!("expected a field store, found {other:?}"),
            }
            assert!(matches!(value.kind, hir::ExprKind::IntLiteral(1)));
        }
        other => panic!("expected an assignment, found {other:?}"),
    }
}

#[test]
fn assigning_a_struct_field_in_a_method_is_an_error() {
    let file = file(vec![
        struct_decl_methods(
            "S",
            vec![("v", ty_named("Int"))],
            vec![method("m", vec![], None, vec![assign("v", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value-type fields stay unwritable");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `v`");
}

#[test]
fn this_outside_a_member_function_is_an_error() {
    let file = file(vec![fun_expr(
        "main",
        vec![],
        vec![],
        Some(ty_named("Any")),
        this_expr(),
    )]);
    let errors = lower_user(file).expect_err("`this` at top level must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`this` is only allowed inside member functions"
    );
}

// --- negative: type operators ---

#[test]
fn ref_eq_on_value_types_is_an_error() {
    let file = file(vec![
        struct_s(),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("S")), ("b", ty_named("S"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefEq, var("a"), var("b")),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("`===` on value types must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "reference equality `===` is not supported on value types"
    );
}

#[test]
fn useless_is_check_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Boolean")),
            is_ty(var("x"), ty_named("String"), false),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a useless check must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "useless type check: `Int` can never be `String`"
    );
}

#[test]
fn impossible_cast_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            cast_ty(var("x"), ty_named("String"), false),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an impossible cast must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cast from `Int` to `String` can never succeed"
    );
}

#[test]
fn mutable_variables_are_not_smart_cast() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                // `var x: Any` — the mutable variable is not narrowed
                // inside the branch, so `x.v` still resolves against
                // `Any`.
                Statement {
                    kind: StatementKind::ValDecl(ValDecl {
                        mutable: true,
                        target: pat_bind("x"),
                        ty: Some(ty_named("Any")),
                        init: struct_init("S", vec![int_lit(1)]),
                        span: sp(),
                    }),
                    span: sp(),
                },
                if_stmt(
                    is_ty(var("x"), ty_named("S"), false),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a mutable variable must not narrow");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Any` has no fields");
}

// --- positive: field assignment ---

#[test]
fn field_assignment_on_a_var_property() {
    let bad = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![
                // `var` property, absolute layout index 2.
                assign_field(var("p"), "y", int_lit(3)),
                // Inherited `val` property would be rejected (see the
                // negative test); the store adapts the value type.
                assign_field(var("p"), "x", int_lit(4)),
            ],
        ),
        fun("main", vec![]),
    ]);
    // `x` is a `val` property — the second store must be rejected.
    let errors = lower_user(bad).expect_err("assigning a val property must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `x`");

    let ok = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![assign_field(var("p"), "y", int_lit(3))],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(ok).expect("a var property store must lower");
    match &body_of(&module, "f").statements[0].kind {
        hir::StatementKind::Assign { target, value } => {
            match target {
                hir::AssignTarget::Field { receiver, field } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert_eq!(
                        *field,
                        hir::FieldRef::ClassField {
                            application: class_application(&module, class_id(&module, "Point")),
                            index: 2
                        }
                    );
                }
                other => panic!("expected a field store, found {other:?}"),
            }
            assert!(matches!(value.kind, hir::ExprKind::IntLiteral(3)));
        }
        other => panic!("expected an assignment, found {other:?}"),
    }
}

#[test]
fn field_assignment_on_a_value_type_is_an_error() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![("s", ty_named("S"))],
            None,
            vec![assign_field(var("s"), "v", int_lit(1))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value-type field stores must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "field assignment is not supported (value types are immutable)"
    );
}

#[test]
fn field_assignment_value_type_is_checked() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![assign_field(var("p"), "y", str_lit("s"))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a wrong value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot assign value of type String to property `y` of type Int"
    );
}

// --- positive: value types implementing interfaces (spec 4.4.3) ---

/// `struct S(val v: Int) : Describable { override fun describe(): String = "S" }`.
fn describable_s() -> Decl {
    struct_decl_full(
        "S",
        vec![("v", ty_named("Int"))],
        vec!["Describable"],
        vec![override_method_expr(
            "describe",
            vec![],
            Some(ty_named("String")),
            str_lit("S"),
        )],
    )
}

#[test]
fn struct_implements_interface_and_boxes() {
    let file = file(vec![
        describable(),
        describable_s(),
        fun(
            "main",
            vec![
                // Value type → its interface: boxed (spec 4.4.4).
                val_ty(
                    "d",
                    Some(ty_named("Describable")),
                    struct_init("S", vec![int_lit(1)]),
                ),
            ],
        ),
        // Interface dispatch on a boxed value type resolves to the
        // interface method (the adjust thunk is mir-lower's job).
        fun_expr(
            "show",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            method_call(var("d"), "describe", vec![]),
        ),
    ]);
    let module = lower_user(file).expect("a value-type implementation must lower");

    // The interface list is recorded on the struct declaration.
    let s_id = module
        .structs
        .iter()
        .find(|(_, s)| s.name == "S")
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(
        module.structs[s_id].interfaces,
        vec![interface_ty(&module, "Describable")]
    );

    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => {
            assert!(matches!(init.kind, hir::ExprKind::Box(_)));
            assert!(matches!(module.types[init.ty], hir::Type::Interface(..)));
        }
        other => panic!("expected a val decl, found {other:?}"),
    }
    match &returned(body_of(&module, "show")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Describable.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn enum_implements_interface() {
    let file = file(vec![
        describable(),
        enum_decl_full(
            "Mark",
            vec![],
            vec![variant_unit("On"), variant_unit("Off")],
            vec!["Describable"],
            vec![override_method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                str_lit("mark"),
            )],
        ),
        fun_expr(
            "f",
            vec![],
            vec![("m", ty_named("Mark"))],
            Some(ty_named("Describable")),
            var("m"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("an enum implementation must lower");
    // Enum → interface at a return position: Box.
    assert!(matches!(
        returned(body_of(&module, "f")).kind,
        hir::ExprKind::Box(_)
    ));
}

#[test]
fn value_type_missing_an_implementation_is_an_error() {
    let file = file(vec![
        describable(),
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec!["Describable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "struct `S` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn enum_missing_an_implementation_is_an_error() {
    let file = file(vec![
        describable(),
        enum_decl_full(
            "Mark",
            vec![],
            vec![variant_unit("On")],
            vec!["Describable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "enum `Mark` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn value_type_implementation_requires_the_override_modifier() {
    let file = file(vec![
        describable(),
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec!["Describable"],
            vec![method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                str_lit("S"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a missing `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`describe` overrides `Describable.describe` and must be marked `override`"
    );
}

#[test]
fn value_type_override_without_an_interface_is_an_error() {
    let file = file(vec![
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec![],
            vec![method_full(
                true,
                false,
                "m",
                vec![],
                None,
                FunctionBody::Block(block(vec![])),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a spurious `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}

#[test]
fn ref_ne_lowers_to_ref_ne() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("Any")), ("s", ty_named("Shape"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefNe, var("a"), var("s")),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("`!==` must lower");
    assert!(matches!(
        returned(body_of(&module, "f")).kind,
        hir::ExprKind::Binary {
            op: hir::BinOp::RefNe,
            ..
        }
    ));
}

// --- qualified enum variant construction in method-call shape ---

/// The M6 parser folds `E.V(args)` into `MethodCall { receiver:
/// Var("E"), ... }`; when `E` is no variable but an enum, this is a
/// qualified variant construction, not a method call.
#[test]
fn qualified_variant_construction_in_method_call_shape() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![
                variant_positional("Circle", vec![ty_named("Int")]),
                variant_constructor("Named", vec![("w", ty_named("Int"), Some(int_lit(7)))]),
            ],
        ),
        fun(
            "main",
            vec![
                val("s", method_call(var("Shape"), "Circle", vec![int_lit(5)])),
                // Constructor-style defaults fill the tail.
                val("n", method_call(var("Shape"), "Named", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("qualified variants must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                assert_eq!(*variant, 0);
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(5)));
            }
            other => panic!("expected a variant construction, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
    match &main.statements[1].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                assert_eq!(*variant, 1);
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(7)));
            }
            other => panic!("expected a variant construction, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
}

#[test]
fn qualified_generic_variant_infers_type_arguments() {
    let file = file(vec![
        enum_decl(
            "Box",
            vec!["T"],
            vec![variant_positional("Wrap", vec![ty_named("T")])],
        ),
        fun(
            "main",
            vec![val_ty(
                "b",
                Some(ty_generic("Box", vec![ty_named("Int")])),
                method_call(var("Box"), "Wrap", vec![int_lit(1)]),
            )],
        ),
    ]);
    let module = lower_user(file).expect("generic variant construction must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::VariantConstruct { application, .. } => {
                let arguments = &module.enum_applications[*application].arguments;
                assert_eq!(arguments.len(), 1);
                assert_eq!(module.types[arguments[0]], hir::Type::Int);
            }
            other => panic!("expected a variant construction, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
}

#[test]
fn a_variable_shadows_the_enum_name() {
    // A local variable named `Shape` wins over the enum: the receiver
    // is a genuine method call on the variable (and fails as one).
    let file = file(vec![
        struct_s(),
        enum_decl(
            "Shape",
            vec![],
            vec![variant_positional("Circle", vec![ty_named("Int")])],
        ),
        fun(
            "main",
            vec![
                val("Shape", struct_init("S", vec![int_lit(1)])),
                val("s", method_call(var("Shape"), "Circle", vec![int_lit(5)])),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("the variable must shadow the enum");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `S` has no method `Circle`");
}

#[test]
fn unknown_qualified_variant_in_method_call_shape_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_positional("Circle", vec![ty_named("Int")])],
        ),
        fun(
            "main",
            vec![val(
                "s",
                method_call(var("Shape"), "Square", vec![int_lit(5)]),
            )],
        ),
    ]);
    let errors = lower_user(file).expect_err("an unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Shape` has no variant `Square`");
}
