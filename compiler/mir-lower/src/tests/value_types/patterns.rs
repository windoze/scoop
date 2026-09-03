use super::super::*;

/// Shared `when` statement builder for the pattern-lowering tests.
fn when_stmt(
    subject: hir::Expr,
    arms: Vec<hir::WhenArm>,
    else_body: Option<Vec<hir::Statement>>,
) -> hir::Statement {
    stmt(hir::StatementKind::When(hir::When {
        subject,
        arms,
        else_body,
    }))
}

fn arm(pattern: hir::Pattern, guard: Option<hir::Expr>, body: Vec<hir::Statement>) -> hir::WhenArm {
    hir::WhenArm {
        pattern,
        guard,
        body,
        span: SPAN,
    }
}

#[test]
fn when_lowers_to_a_decision_sequence() {
    // val o = Some(1); when (o) { Some(x) -> print(x); None -> println("none") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                when_stmt(
                    local_ref(o, option_int),
                    vec![
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 0,
                                fields: vec![(0, hir::Pattern::Binding { local: x })],
                            },
                            None,
                            vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                        ),
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 1,
                                fields: Vec::new(),
                            },
                            None,
                            vec![expr_stmt(call(
                                &h,
                                println_string,
                                vec![str_lit(&h, "none")],
                            ))],
                        ),
                    ],
                    None,
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subject is evaluated once into `$when.1`; each arm is a
    // tag comparison, then the field bindings, then the body; a
    // failed tag test falls through to the next arm. (`print` /
    // `println` are ordinary core functions — M7 — so the arms
    // call the overloads, not runtime shims.)
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  extern ef1 coreIntToString @scoop_rt_int_to_string(Int) -> String <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = extern1 @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      call @scoop.print direct
        Type Int
        Local x
      goto bb3
    bb2 if.else.2
      branch bb4 bb5
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 1
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb5
    bb5 if.merge.5
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn a_failed_guard_falls_through_to_the_next_arm() {
    // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let else_body = || {
        vec![expr_stmt(call(
            &h,
            println_string,
            vec![str_lit(&h, "neg")],
        ))]
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(o, option_int),
                vec![arm(
                    hir::Pattern::Variant {
                        application: option_application,
                        variant: 0,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    Some(binary(
                        hir::BinOp::Gt,
                        local_ref(x, int),
                        int_lit(&h, 0),
                        h.boolean,
                    )),
                    vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                )],
                Some(else_body()),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // The guard nests inside the tag test's then branch; failing
    // it falls through to the next arm — the `else` body here,
    // which is lowered once per fallthrough edge.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  extern ef1 coreIntToString @scoop_rt_int_to_string(Int) -> String <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = extern1 @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      branch bb4 bb5
        Type Boolean
        Binary IntGt
          Type Int
          Local x
          Type Int
          IntLiteral 0
    bb2 if.else.2
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      goto bb3
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.print direct
        Type Int
        Local x
      goto bb6
    bb5 if.else.5
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb6
    bb6 if.merge.6
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"neg\"
  str @scoop.str.2 \"neg\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn literal_patterns_match_by_equality() {
    // when (n) { 1 -> println("one"); else -> println("other") }
    let mut h = Harness::new();
    let println = h.println_string();
    let int = h.int;
    let boolean = h.boolean;
    let mut equals_locals = Arena::new();
    let left = equals_locals.alloc(local("left", int));
    let right = equals_locals.alloc(local("right", int));
    let equals = h.user_fn_full(
        "Int.equals",
        Vec::new(),
        vec![param("left", int, left), param("right", int, right)],
        boolean,
        hir::Body {
            locals: equals_locals,
            statements: vec![hir::Statement {
                kind: hir::StatementKind::Return {
                    value: Some(bool_lit(&h, true)),
                },
                span: SPAN,
            }],
        },
    );
    let mut locals = Arena::new();
    let n = locals.alloc(local("n", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(n, int),
                vec![arm(
                    hir::Pattern::Literal {
                        value: int_lit(&h, 1),
                        equals: hir::Callable::Function(equals),
                        subject_ty: int,
                    },
                    None,
                    vec![expr_stmt(call(&h, println, vec![str_lit(&h, "one")]))],
                )],
                Some(vec![expr_stmt(call(
                    &h,
                    println,
                    vec![str_lit(&h, "other")],
                ))]),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let (call, result) = entry_statements(body)
        .iter()
        .find_map(|statement| {
            matches!(statement.kind, mir::StatementKind::Call(_)).then(|| statement_call(statement))
        })
        .expect("literal pattern calls its HIR-selected equality target");
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    assert!(matches!(call.args.as_slice(), [scrutinee, literal]
            if matches!(scrutinee.kind, mir::ExprKind::Local(_))
                && matches!(literal.kind, mir::ExprKind::IntLiteral(1))));
    let result = result.expect("equals returns Boolean");
    let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
        panic!("literal equality result controls the pattern branch")
    };
    assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == result));
}

#[test]
fn destructuring_val_declarations_extract_bindings() {
    // val (a, b) = (1, "x"); val Point { x, .. } = p
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let point = h.strukt("Point", &[("x", int), ("y", int)]);
    let point_ty = h.struct_ty(point);
    let point_application = h.struct_application_of(point_ty);
    let pair = h.tuple(&[int, string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", int));
    let b = locals.alloc(local("b", string));
    let p = locals.alloc(local("p", point_ty));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Tuple(vec![
                        hir::Pattern::Binding { local: a },
                        hir::Pattern::Binding { local: b },
                    ]),
                    init: expr(
                        hir::ExprKind::TupleLiteral(vec![int_lit(&h, 1), str_lit(&h, "x")]),
                        pair,
                    ),
                }),
                val_decl(
                    p,
                    struct_init(&h, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                ),
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Struct {
                        application: point_application,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    init: local_ref(p, point_ty),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // Each destructuring declaration evaluates its init once into
    // a hidden local, then binds the extracted fields.
    let expected = "\
Module
  struct Point (x: Int, y: Int)
  fun main @scoop_main() -> Unit
    bb0 entry
      val $bind.1: (Int, String)
        Type (Int, String)
        TupleLiteral
          Type Int
          IntLiteral 1
          Type String
          StringConst @scoop.str.0
      val a: Int
        Type Int
        FieldAccess 0
          Type (Int, String)
          Local $bind.1
      val b: String
        Type String
        FieldAccess 1
          Type (Int, String)
          Local $bind.1
      val p: Point
        Type Point
        StructInit Point
          Type Int
          IntLiteral 3
          Type Int
          IntLiteral 4
      val $bind.2: Point
        Type Point
        Local p
      val x: Int
        Type Int
        FieldAccess 0
          Type Point
          Local $bind.2
      return
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}
