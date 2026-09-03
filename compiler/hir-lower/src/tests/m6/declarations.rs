use super::*;

// --- positive: golden dump ---

#[test]
fn class_hierarchy_golden() {
    let file = file(vec![describable(), shape(), point(), fun("main", vec![])]);
    let module = lower_user(file).expect("the hierarchy must lower");
    let expected = r#"Module
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
    val local1
      Local value : T0
    val local2
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local $receiver : T0
    val local3
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
    return
  fun println<T : ToString>(value: T0): Unit
    val local1
      Local value : T0
    val local2
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local $receiver : T0
    val local3
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
    val local4
      StringLiteral "\n" : String
    val local5
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
  fun main(): Unit
  entry main
  instance println<Int>
"#;
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
    let (base, delegation) = point.base_class.as_ref().expect("Point has a base");
    assert!(matches!(module.types[*base], hir::Type::Class(application)
        if module.class_applications[application].template == shape_id
            && module.class_applications[application].arguments.is_empty()));
    assert_eq!(delegation.args.len(), 1);
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::StringLiteral(_),
                ..
            },
            ..
        }
    )));

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
