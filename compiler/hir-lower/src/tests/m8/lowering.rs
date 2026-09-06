use super::*;

#[test]
fn try_catch_finally_golden() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "read",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                throw_stmt(call("UnwrapException", vec![])),
                ret(Some(int_lit(1))),
            ],
        ),
        fun(
            "main",
            vec![
                try_stmt(
                    vec![
                        stmt(call("read", vec![])),
                        stmt(call("println", vec![str_lit("unreachable")])),
                    ],
                    vec![
                        catch_clause(
                            "e",
                            ty_named("UnwrapException"),
                            vec![stmt(call("println", vec![str_lit("caught unwrap")]))],
                        ),
                        catch_clause(
                            "e",
                            ty_named("Exception"),
                            vec![stmt(call("println", vec![str_lit("caught other")]))],
                        ),
                    ],
                    Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
                ),
                try_stmt(
                    vec![throw_stmt(call("MyError", vec![int_lit(42)]))],
                    vec![catch_clause(
                        "e",
                        ty_named("MyError"),
                        vec![stmt(call("println", vec![field(var("e"), "code")]))],
                    )],
                    Some(vec![stmt(call("println", vec![str_lit("done")]))]),
                ),
            ],
        ),
    ]);
    let module = lower_user_with_exceptions(file).expect("the exception program must lower");
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property9: Option<String>
    property9 val message: Option<String> getter9=storage <stored field0 init=parameter9>
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException(message: Option<String>)
  class MyError(code: Int)
    field1 property10: Int
    property10 val code: Int getter10=storage <stored field1 init=parameter11>
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Long
  interface Iterator<T>
    fun next(): Option<T0>
  interface Iterable<T>
    operator fun iterator(): Iterator<T0>
  interface Continuation<T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<T>
    suspend fun run(): T0
  interface SuspendRegistration<T>
    fun register(continuation: Continuation<T0>): Unit
  fun coreBooleanEquals(arg1: Boolean, arg2: Boolean): Boolean <extern0 abi=scoop symbol=scoop_rt_bool_equals>
  fun coreStringEquals(arg1: String, arg2: String): Boolean <extern1 abi=scoop symbol=scoop_rt_string_eq>
  fun coreLongToString(arg1: Long): String <extern2 abi=scoop symbol=scoop_rt_long_to_string>
  fun coreULongToString(arg1: ULong): String <extern3 abi=scoop symbol=scoop_rt_ulong_to_string>
  fun coreBooleanToString(arg1: Boolean): String <extern4 abi=scoop symbol=scoop_rt_bool_to_string>
  fun coreLongHash(arg1: Long): Long <extern5 abi=scoop symbol=scoop_rt_long_hash>
  fun coreULongHash(arg1: ULong): Long <extern6 abi=scoop symbol=scoop_rt_ulong_hash>
  fun coreBooleanHash(arg1: Boolean): Long <extern7 abi=scoop symbol=scoop_rt_bool_hash>
  fun coreStringHash(arg1: String): Long <extern8 abi=scoop symbol=scoop_rt_string_hash>
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(arg1: String): Unit <extern9 abi=scoop symbol=scoop_rt_write>
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
  fun read(): Int
    throw
      ClassInit UnwrapException : UnwrapException
    return
      IntegerLiteral 1 : Int
  fun main(): Unit
    try
      Call read : Int
      val local0
        StringLiteral "unreachable" : String
      val local1
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    catch e: UnwrapException
      val local3
        StringLiteral "caught unwrap" : String
      val local4
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    catch e: Exception
      val local6
        StringLiteral "caught other" : String
      val local7
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    finally
      val local8
        StringLiteral "finally" : String
      val local9
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    try
      val local10
        IntegerLiteral 42 : Int
      val local11
        Local $argument.0 : Int
      throw
        ClassInit MyError : MyError
          Local $parameter.code : Int
    catch e: MyError
      val local13
        FieldAccess class field code : Int
          Local e : MyError
      val local14
        Local $argument.0 : Int
      Call println<Int> : Unit
        Local $parameter.value : Int
    finally
      val local15
        StringLiteral "done" : String
      val local16
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
  entry main
  instance println<String>
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: structure and scoping ---

/// The catch local is an immutable local of the clause type, visible
/// in its own clause body only; sibling clauses may reuse the name.
#[test]
fn catch_local_structure() {
    let file = file(vec![fun(
        "main",
        vec![try_stmt(
            vec![throw_stmt(call("Throwable", vec![]))],
            vec![
                catch_clause("e", ty_named("UnwrapException"), vec![]),
                catch_clause("e", ty_named("Exception"), vec![]),
            ],
            None,
        )],
    )]);
    let module = lower_user_with_exceptions(file).expect("the try must lower");
    let main = &module.functions[module.entry];
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main has a user body");
    };
    let hir::StatementKind::Try(try_) = &body.statements[0].kind else {
        panic!("expected a try statement");
    };
    assert_eq!(try_.catches.len(), 2);
    let first = &try_.catches[0];
    assert_eq!(body.locals[first.local].name, "e");
    assert!(!body.locals[first.local].mutable);
    assert_eq!(body.locals[first.local].ty, first.ty);
    assert!(matches!(
        module.types[first.ty],
        hir::Type::Class(application)
            if module.classes[module.class_applications[application].template].name
                == "UnwrapException"
                && module.class_applications[application].arguments.is_empty()
    ));
}

/// Custom exception classes extend the `open` core `Exception` (the
/// standard M6 inheritance rule — no exception-hierarchy special
/// cases).
#[test]
fn custom_exceptions_extend_open_exception() {
    let file = file(vec![
        custom_error(),
        fun(
            "main",
            vec![
                val_ty(
                    "e",
                    Some(ty_named("Throwable")),
                    call("MyError", vec![int_lit(7)]),
                ),
                throw_stmt(var("e")),
            ],
        ),
    ]);
    lower_user_with_exceptions(file).expect("exception subclasses must lower");
}

/// The built-in exception subclasses are final: inheriting one is an
/// error, exactly like any other final class.
#[test]
fn inheriting_a_final_exception_subclass_is_an_error() {
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Bad",
            vec![],
            Some(("UnwrapException", vec![])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_exceptions(file).expect_err("inheriting a final class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `UnwrapException` is final and cannot be inherited"
    );
}

#[test]
fn bare_class_supertype_is_classified_as_the_base() {
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "E",
            vec![(false, "message", ty_nullable(ty_named("String")))],
            None,
            vec!["Throwable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user_with_exceptions(file).expect("a bare class supertype must lower");
    let (_, class) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "E")
        .expect("E is declared");
    let base = class.base_class.as_ref().expect("E has a base class");
    assert!(matches!(module.types[*base], hir::Type::Class(application)
        if module.classes[module.class_applications[application].template].name == "Throwable"));
    let hir::ClassConstructorKind::Primary {
        base:
            hir::BaseInitialization::Super {
                arguments: delegation,
                ..
            },
        ..
    } = &module.class_constructors[class.constructors[0]].kind
    else {
        panic!("E delegates to Throwable")
    };
    assert!(delegation.args.is_empty());
}

// --- negative: throw ---
