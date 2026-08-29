//! M2 tests: value types (struct / tuple), locals and scopes,
//! control flow, operators — one negative test per diagnostic.

use super::*;

// --- positive: structs ---

#[test]
fn struct_construction_and_field_access() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val("x", field(var("p"), "x")),
                stmt(call("println", vec![var("x")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("struct program must lower");

    // The struct type is allocated right after the well-known types.
    assert!(matches!(module.types[module.int], Type::Int));
    let expected = "\
Module
  struct Point
    field x: Int
    field y: Int
  enum Option<T>
    Some(_1: T0)
    None()
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
  fun main(): Unit
    val local0
      StructInit Point : Point
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    val local1
      FieldAccess field 0 : Int
        Local p : Point
    Call println : Unit
      Box : Any
        Local x : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn struct_init_node_also_constructs() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![val("p", struct_init("Point", vec![int_lit(1)]))],
        ),
    ]);
    let module = lower_user(file).expect("StructInit node must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
}

/// A struct and a function may share a name (separate namespaces). In
/// `Name(...)` call position the struct namespace wins: the expression
/// is a construction, not a call.
#[test]
fn struct_shadows_function_in_call_position() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun("Point", vec![]),
        fun("main", vec![val("p", call("Point", vec![int_lit(5)]))]),
    ]);
    let module = lower_user(file).expect("struct/function name sharing must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
    assert!(dump.contains("fun Point"), "{dump}");
}

#[test]
fn struct_fields_may_reference_later_structs() {
    let file = file(vec![
        struct_decl("A", vec![("b", ty_named("B"))]),
        struct_decl("B", vec![("v", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("a", call("A", vec![call("B", vec![int_lit(1)])])),
                val("v", field(field(var("a"), "b"), "v")),
                stmt(call("println", vec![var("v")])),
            ],
        ),
    ]);
    lower_user(file).expect("forward struct references must lower");
}

// --- positive: tuples ---

#[test]
fn tuples_and_indexing() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("q", tuple_lit(vec![int_lit(1), str_lit("hello")])),
            val("s", index(var("q"), 2)),
            stmt(call("println", vec![var("s")])),
            val("u", unit_lit()),
            val_ty(
                "t",
                Some(ty_tuple(vec![ty_named("Int")])),
                tuple_lit(vec![int_lit(42)]),
            ),
            stmt(call("print", vec![index(var("t"), 1)])),
        ],
    )]);
    let module = lower_user(file).expect("tuple program must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
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
  fun main(): Unit
    val local0
      TupleLiteral : (Int, String)
        IntLiteral 1 : Int
        StringLiteral \"hello\" : String
    val local1
      FieldAccess _2 : String
        Local q : (Int, String)
    Call println : Unit
      Local s : Any
    val local2
      UnitLiteral : Unit
    val local3
      TupleLiteral : (Int)
        IntLiteral 42 : Int
    Call print : Unit
      Box : Any
        FieldAccess _1 : Int
          Local t : (Int)
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn structs_and_tuples_nest() {
    let file = file(vec![
        struct_decl(
            "Wrap",
            vec![("pair", ty_tuple(vec![ty_named("Int"), ty_named("String")]))],
        ),
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val(
                    "w",
                    call("Wrap", vec![tuple_lit(vec![int_lit(1), str_lit("one")])]),
                ),
                val("n", index(field(var("w"), "pair"), 1)),
                val(
                    "p",
                    tuple_lit(vec![
                        call("Point", vec![int_lit(1), int_lit(2)]),
                        int_lit(3),
                    ]),
                ),
                val("q", index(var("p"), 1)),
                val(
                    "same",
                    binary(
                        BinOp::Eq,
                        var("q"),
                        call("Point", vec![int_lit(1), int_lit(2)]),
                    ),
                ),
                val(
                    "same2",
                    binary(
                        BinOp::Eq,
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                    ),
                ),
                stmt(call("println", vec![var("n")])),
                stmt(call("println", vec![var("same")])),
                stmt(call("println", vec![var("same2")])),
            ],
        ),
    ]);
    lower_user(file).expect("nested struct/tuple program must lower");
}

// --- positive: operators and control flow ---

#[test]
fn var_rebinding_and_control_flow() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("n", int_lit(0)),
            while_stmt(
                binary(BinOp::Lt, var("n"), int_lit(3)),
                vec![assign("n", binary(BinOp::Add, var("n"), int_lit(1)))],
            ),
            if_stmt(
                binary(
                    BinOp::And,
                    binary(BinOp::Eq, var("n"), int_lit(3)),
                    bool_lit(true),
                ),
                vec![stmt(call("println", vec![str_lit("ok")]))],
                Some(vec![stmt(call("println", vec![str_lit("ng")]))]),
            ),
        ],
    )]);
    let module = lower_user(file).expect("control flow program must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
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
  fun main(): Unit
    val local0
      IntLiteral 0 : Int
    while
      Binary Lt : Boolean
        Local n : Int
        IntLiteral 3 : Int
      assign n
        Binary Add : Int
          Local n : Int
          IntLiteral 1 : Int
    if
      Binary And : Boolean
        Binary Eq : Boolean
          Local n : Int
          IntLiteral 3 : Int
        BoolLiteral true : Boolean
      Call println : Unit
        StringLiteral \"ok\" : Any
    else
      Call println : Unit
        StringLiteral \"ng\" : Any
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn unary_operators_and_all_printables() {
    let file = file(vec![fun(
        "main",
        vec![
            val("n", unary(UnOp::Neg, int_lit(5))),
            val("b", unary(UnOp::Not, bool_lit(false))),
            val(
                "m",
                binary(
                    BinOp::Sub,
                    binary(BinOp::Mul, var("n"), int_lit(2)),
                    int_lit(1),
                ),
            ),
            val("d", binary(BinOp::Div, var("m"), int_lit(3))),
            val("le", binary(BinOp::Le, var("d"), int_lit(0))),
            val("ge", binary(BinOp::Ge, var("d"), int_lit(0))),
            val("gt", binary(BinOp::Gt, var("d"), int_lit(0))),
            val("ne", binary(BinOp::Ne, var("le"), var("ge"))),
            val("or", binary(BinOp::Or, var("gt"), var("ne"))),
            stmt(call("println", vec![int_lit(42)])),
            stmt(call("println", vec![bool_lit(true)])),
            stmt(call("print", vec![var("or")])),
        ],
    )]);
    lower_user(file).expect("operator program must lower");
}

// --- positive: scopes ---

#[test]
fn inner_scopes_shadow_and_do_not_leak() {
    let file = file(vec![fun(
        "main",
        vec![
            val("x", int_lit(1)),
            if_stmt(
                bool_lit(true),
                vec![
                    val("x", str_lit("inner")),
                    stmt(call("println", vec![var("x")])),
                ],
                None,
            ),
            stmt(call("println", vec![var("x")])),
            block_stmt(vec![
                val("y", int_lit(2)),
                stmt(call("println", vec![var("y")])),
            ]),
        ],
    )]);
    let module = lower_user(file).expect("shadowing program must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
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
  fun main(): Unit
    val local0
      IntLiteral 1 : Int
    if
      BoolLiteral true : Boolean
      val local1
        StringLiteral \"inner\" : String
      Call println : Unit
        Local x : Any
    Call println : Unit
      Box : Any
        Local x : Int
    val local2
      IntLiteral 2 : Int
    Call println : Unit
      Box : Any
        Local y : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

// --- negative: struct declarations ---

#[test]
fn duplicate_struct_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![]),
        struct_decl("Point", vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate struct `Point`");
}

#[test]
fn duplicate_field_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("x", ty_named("Int"))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate field `x` in struct `Point`");
}

#[test]
fn unknown_field_type_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("z", ty_named("Foo"))]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unknown field type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Foo`");
}

// --- negative: struct construction ---

#[test]
fn struct_init_arity_is_an_error() {
    for (args, expected, supplied) in [
        (vec![int_lit(1)], 2, 1),
        (vec![int_lit(1), int_lit(2), int_lit(3)], 2, 3),
    ] {
        let file = file(vec![
            struct_decl(
                "Point",
                vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            ),
            fun("main", vec![val("p", call("Point", args))]),
        ]);
        let errors = lower_user(file).expect_err("wrong arity must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!(
                "struct `Point` takes exactly {expected} arguments, but {supplied} were supplied"
            )
        );
    }
}

#[test]
fn struct_init_arity_singular_noun() {
    let file = file(vec![
        struct_decl("Box", vec![("v", ty_named("Int"))]),
        fun("main", vec![val("b", call("Box", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "struct `Box` takes exactly 1 argument, but 0 were supplied"
    );
}

#[test]
fn struct_init_field_type_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![val("p", call("Point", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("field type mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `y` of `Point` must be of type Int, found String"
    );
}

#[test]
fn unknown_struct_init_node_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("p", struct_init("Foo", vec![]))],
    )]);
    let errors = lower_user(file).expect_err("unknown struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown struct `Foo`");
}

// --- negative: declarations and assignments ---

#[test]
fn initializer_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("x", Some(ty_named("Int")), str_lit("s"))],
    )]);
    let errors = lower_user(file).expect_err("annotation mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `x` must be of type Int, found String"
    );
}

#[test]
fn unknown_annotation_type_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("x", Some(ty_named("Foo")), int_lit(1))],
    )]);
    let errors = lower_user(file).expect_err("unknown annotation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Foo`");
}

#[test]
fn redeclaration_in_same_scope_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", int_lit(1)), val("x", int_lit(2))],
    )]);
    let errors = lower_user(file).expect_err("redeclaration must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`x` is already declared in this scope");
}

#[test]
fn assign_to_immutable_variable_is_an_error_with_target_span() {
    let target_span = Span::new(20, 21);
    let mut assignment = assign("x", int_lit(2));
    if let StatementKind::Assign(a) = &mut assignment.kind {
        let ast::AssignTarget::Local(name) = &mut a.target else {
            panic!("the assign builder produces a local target");
        };
        name.span = target_span;
    }
    let file = file(vec![fun("main", vec![val("x", int_lit(1)), assignment])]);
    let errors = lower_user(file).expect_err("assigning to a val must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable variable `x`");
    assert_eq!(errors[0].span, Some(target_span));
}

#[test]
fn assign_to_unknown_variable_is_an_error() {
    let file = file(vec![fun("main", vec![assign("x", int_lit(2))])]);
    let errors = lower_user(file).expect_err("assigning to an unknown name must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `x`");
}

#[test]
fn assign_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![var_("x", int_lit(1)), assign("x", str_lit("s"))],
    )]);
    let errors = lower_user(file).expect_err("assignment mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot assign value of type String to `x` of type Int"
    );
}

#[test]
fn variable_out_of_scope_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            block_stmt(vec![val("y", int_lit(1))]),
            stmt(call("println", vec![var("y")])),
        ],
    )]);
    let errors = lower_user(file).expect_err("out-of-scope reference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}

#[test]
fn if_body_does_not_leak_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            if_stmt(bool_lit(true), vec![val("y", int_lit(1))], None),
            stmt(call("println", vec![var("y")])),
        ],
    )]);
    let errors = lower_user(file).expect_err("out-of-scope reference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}

#[test]
fn unknown_variable_read_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call("println", vec![var("y")]))],
    )]);
    let errors = lower_user(file).expect_err("unknown variable must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}

// --- negative: operators ---

#[test]
fn arithmetic_requires_int_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Add, int_lit(1), str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int arithmetic must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `+` requires Int operands, found Int and String"
    );
}

#[test]
fn comparison_requires_int_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Lt, str_lit("a"), str_lit("b")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int comparison must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `<` requires Int operands, found String and String"
    );
}

#[test]
fn equality_requires_matching_types() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::Eq, int_lit(1), str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("mismatched equality must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `==` requires operands of the same type, found Int and String"
    );
}

#[test]
fn logical_and_requires_boolean_operands() {
    let file = file(vec![fun(
        "main",
        vec![val("x", binary(BinOp::And, int_lit(1), bool_lit(true)))],
    )]);
    let errors = lower_user(file).expect_err("non-Boolean `&&` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `&&` requires Boolean operands, found Int and Boolean"
    );
}

#[test]
fn negation_requires_an_int_operand() {
    let file = file(vec![fun(
        "main",
        vec![val("x", unary(UnOp::Neg, str_lit("s")))],
    )]);
    let errors = lower_user(file).expect_err("non-Int negation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `-` requires an Int operand, found String"
    );
}

#[test]
fn logical_not_requires_a_boolean_operand() {
    let file = file(vec![fun(
        "main",
        vec![val("x", unary(UnOp::Not, int_lit(1)))],
    )]);
    let errors = lower_user(file).expect_err("non-Boolean `!` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `!` requires a Boolean operand, found Int"
    );
}

// --- negative: conditions ---

#[test]
fn if_condition_must_be_boolean() {
    let file = file(vec![fun("main", vec![if_stmt(int_lit(1), vec![], None)])]);
    let errors = lower_user(file).expect_err("Int condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "if condition must be Boolean, found Int");
}

#[test]
fn while_condition_must_be_boolean() {
    let file = file(vec![fun("main", vec![while_stmt(str_lit("x"), vec![])])]);
    let errors = lower_user(file).expect_err("String condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "while condition must be Boolean, found String"
    );
}

// --- negative: field access ---

#[test]
fn unknown_struct_field_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("z", field(var("p"), "z")),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `z`");
}

#[test]
fn struct_index_access_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("z", index(var("p"), 1)),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("index on struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `_1`");
}

#[test]
fn tuple_index_out_of_bounds_is_an_error() {
    for n in [0, 3] {
        let file = file(vec![fun(
            "main",
            vec![
                val("q", tuple_lit(vec![int_lit(1), str_lit("s")])),
                val("z", index(var("q"), n)),
            ],
        )]);
        let errors = lower_user(file).expect_err("out-of-bounds index must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!("tuple type `(Int, String)` has no element `_{n}`")
        );
    }
}

#[test]
fn tuple_named_field_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("q", tuple_lit(vec![int_lit(1), str_lit("s")])),
            val("z", field(var("q"), "x")),
        ],
    )]);
    let errors = lower_user(file).expect_err("named access on tuple must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple type `(Int, String)` has no field `x`"
    );
}

#[test]
fn field_access_on_scalar_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("n", int_lit(1)), val("z", field(var("n"), "x"))],
    )]);
    let errors = lower_user(file).expect_err("field access on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Int` has no fields");
}

// --- negative: statements ---

#[test]
fn struct_init_statement_is_not_a_call() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun("main", vec![stmt(call("Point", vec![int_lit(1)]))]),
    ]);
    let errors = lower_user(file).expect_err("construction statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
}

#[test]
fn empty_tuple_literal_is_an_error() {
    let file = file(vec![fun("main", vec![val("t", tuple_lit(vec![]))])]);
    let errors = lower_user(file).expect_err("empty tuple literal must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple literal must contain at least one element"
    );
}
