use super::super::*;

/// Shared `when` statement builder for the pattern-lowering tests.
fn when_stmt(
    subject: hir::Expr,
    arms: Vec<hir::WhenArm>,
    fallback: hir::WhenFallback,
) -> hir::Statement {
    stmt(hir::StatementKind::When(hir::When {
        subject,
        arms,
        fallback,
    }))
}

fn arm(pattern: hir::Pattern, guard: Option<hir::Expr>, body: Vec<hir::Statement>) -> hir::WhenArm {
    hir::WhenArm {
        pattern,
        guard: guard.map(|condition| hir::WhenGuard {
            setup: Vec::new(),
            condition,
        }),
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
                    hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix {
                        subject_ty: option_int,
                        application: option_application,
                    }),
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
Module mangling=compact-v2
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  extern ef1 coreLongToString @scoop_rt_long_to_string(Long) -> String <abi=scoop managed>
  enum Option$I32
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = extern1 @scoop_rt_long_to_string direct
        Type Long
        IntegerConversion Int -> Long
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
      val o: Option$I32<Int>
        Type Option$I32<Int>
        VariantConstruct Option$I32<Int> v0
          Type Int
          IntegerLiteral Int value=1 bits=0x00000001
      val $when.1: Option$I32<Int>
        Type Option$I32<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary MachineEq(EnumTag)
          Type machine<enum-tag>
          EnumTag
            Type Option$I32<Int>
            Local $when.1
          Type machine<enum-tag>
          MachineScalarLiteral EnumTag(0)
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I32<Int>
          Local $when.1
      call @scoop.print direct
        Type Int
        Local x
      goto bb3
    bb2 if.else.2
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb3
    bb3 if.merge.3
      return
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn recursive_fields_guard_payload_projection_and_evaluate_the_subject_once() {
    let mut h = Harness::new();
    let int = h.int;
    let inner = h.option(int);
    let inner_application = h.enum_application_of(inner);
    let record = h.strukt("Record", &[("nested", inner), ("ignored", int)]);
    let record_ty = h.struct_ty(record);
    let record_application = h.struct_application_of(record_ty);
    let outer = h.option(record_ty);
    let outer_application = h.enum_application_of(outer);

    let mut producer_locals = Arena::new();
    let input = producer_locals.alloc(local("input", outer));
    let producer = h.user_fn_full(
        "subject",
        Vec::new(),
        vec![param("input", outer, input)],
        outer,
        hir::Body {
            locals: producer_locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(input, outer)),
            })],
        },
    );

    let mut locals = Arena::new();
    let source = locals.alloc(local("source", outer));
    let value = locals.alloc(local("value", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                call_typed(producer, vec![local_ref(source, outer)], outer),
                vec![arm(
                    hir::Pattern::Variant {
                        application: outer_application,
                        variant: 0,
                        fields: vec![(
                            0,
                            hir::Pattern::Struct {
                                application: record_application,
                                fields: vec![
                                    (
                                        0,
                                        hir::Pattern::Variant {
                                            application: inner_application,
                                            variant: 0,
                                            fields: vec![(
                                                0,
                                                hir::Pattern::Binding { local: value },
                                            )],
                                        },
                                    ),
                                    (1, hir::Pattern::Wildcard),
                                ],
                            },
                        )],
                    },
                    None,
                    Vec::new(),
                )],
                hir::WhenFallback::Else(Vec::new()),
            )],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let producer = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "subject").then_some(id))
        .expect("subject producer must lower");

    let subject_calls = body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter(|statement| {
            let mir::StatementKind::Call(mir::CallEffect::Value { call, .. }) = &statement.kind
            else {
                return false;
            };
            matches!(&call.target.callee, mir::Callee::User(function) if *function == producer)
        })
        .count();
    assert_eq!(subject_calls, 1, "the when subject call is emitted once");

    let entry = &body.blocks[body.entry];
    let mir::Terminator::Branch {
        cond,
        then_block: payload_block,
        ..
    } = &entry.terminator
    else {
        panic!("outer enum tag must control the short-circuit edge")
    };
    assert!(matches!(
        &cond.kind,
        mir::ExprKind::Binary { lhs, .. }
            if matches!(&lhs.kind, mir::ExprKind::EnumTag(operand)
                if matches!(&operand.kind, mir::ExprKind::Local(_)))
    ));

    let payload_block = &body.blocks[*payload_block];
    assert!(
        payload_block.statements.iter().any(|statement| {
            let mir::StatementKind::Assign { value, .. } = &statement.kind else {
                return false;
            };
            matches!(
                &value.kind,
                mir::ExprKind::Binary { lhs, .. }
                    if matches!(&lhs.kind, mir::ExprKind::EnumTag(operand)
                        if matches!(&operand.kind, mir::ExprKind::FieldAccess { receiver, index: 0 }
                            if matches!(&receiver.kind, mir::ExprKind::EnumField {
                                variant: 0,
                                index: 0,
                                ..
                            })))
            )
        }),
        "outer payload projection must occur only on the matching-tag edge"
    );
}

#[test]
fn final_refutable_arm_is_unconditional_only_with_an_impossible_proof() {
    fn branch_count(explicit_else: bool) -> usize {
        let mut h = Harness::new();
        let int = h.int;
        let option_int = h.option(int);
        let application = h.enum_application_of(option_int);
        let mut locals = Arena::new();
        let subject = locals.alloc(local("subject", option_int));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(subject, option_int),
                    vec![
                        arm(
                            hir::Pattern::Variant {
                                application,
                                variant: 0,
                                fields: vec![(0, hir::Pattern::Wildcard)],
                            },
                            None,
                            Vec::new(),
                        ),
                        arm(
                            hir::Pattern::Variant {
                                application,
                                variant: 1,
                                fields: Vec::new(),
                            },
                            None,
                            Vec::new(),
                        ),
                    ],
                    if explicit_else {
                        hir::WhenFallback::Else(Vec::new())
                    } else {
                        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix {
                            subject_ty: option_int,
                            application,
                        })
                    },
                )],
            },
        );
        let module = lower(&h.finish(main));
        module.functions[module.entry]
            .body
            .blocks
            .iter()
            .filter(|(_, block)| matches!(&block.terminator, mir::Terminator::Branch { .. }))
            .count()
    }

    assert_eq!(branch_count(false), 1, "proof removes the final false edge");
    assert_eq!(
        branch_count(true),
        2,
        "an explicit else keeps the final pattern test even when its body is empty",
    );
}

#[test]
fn zero_arm_impossible_when_terminates_with_unreachable() {
    let mut h = Harness::new();
    let never = h.declare_enum("Never", Vec::new(), Vec::new(), Vec::new());
    let never_ty = h.enum_ty(never);
    let application = h.enum_application_of(never_ty);
    let mut locals = Arena::new();
    let subject = locals.alloc(local("subject", never_ty));
    let main = h.user_fn_full(
        "main",
        Vec::new(),
        vec![param("subject", never_ty, subject)],
        h.unit,
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(subject, never_ty),
                Vec::new(),
                hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix {
                    subject_ty: never_ty,
                    application,
                }),
            )],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Unreachable
    ));
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
    let threshold = locals.alloc(local("threshold", int));
    let compared = integer_binary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::NoGcIntegerOperation::CompareTo,
        local_ref(x, int),
        local_ref(threshold, int),
    );
    let mut guarded_arm = arm(
        hir::Pattern::Variant {
            application: option_application,
            variant: 0,
            fields: vec![(0, hir::Pattern::Binding { local: x })],
        },
        Some(binary(
            hir::BinOp::Gt,
            compared,
            integer_lit(&h, hir::IntegerKind::SIGNED_64, 0),
            h.boolean,
        )),
        vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
    );
    guarded_arm
        .guard
        .as_mut()
        .expect("guard")
        .setup
        .push(val_decl(threshold, int_lit(&h, 0)));
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
                vec![guarded_arm],
                hir::WhenFallback::Else(else_body()),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // The guard nests inside the tag test's then branch; failing
    // it falls through to the next arm — the `else` body here,
    // which is lowered once per fallthrough edge.
    let expected = "\
Module mangling=compact-v2
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  extern ef1 coreLongToString @scoop_rt_long_to_string(Long) -> String <abi=scoop managed>
  enum Option$I32
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = extern1 @scoop_rt_long_to_string direct
        Type Long
        IntegerConversion Int -> Long
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
      val $when.1: Option$I32<Int>
        Type Option$I32<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary MachineEq(EnumTag)
          Type machine<enum-tag>
          EnumTag
            Type Option$I32<Int>
            Local $when.1
          Type machine<enum-tag>
          MachineScalarLiteral EnumTag(0)
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I32<Int>
          Local $when.1
      val threshold: Int
        Type Int
        IntegerLiteral Int value=0 bits=0x00000000
      branch bb4 bb5
        Type Boolean
        IntegerCompare greater-than operands=Long result=Boolean
          Type Long
          IntegerCompareTo operands=Int result=Long
            Type Int
            Local x
            Type Int
            Local threshold
          Type Long
          IntegerLiteral Long value=0 bits=0x0000000000000000
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
fn ordinary_literal_patterns_call_the_selected_string_and_boolean_equality() {
    fn lower_case(string: bool) -> (mir::Module, &'static str) {
        let mut h = Harness::new();
        let ty = if string { h.string } else { h.boolean };
        let name = if string {
            "String.equals"
        } else {
            "Boolean.equals"
        };
        let literal = if string {
            str_lit(&h, "one")
        } else {
            bool_lit(&h, true)
        };
        let mut equals_locals = Arena::new();
        let left = equals_locals.alloc(local("left", ty));
        let right = equals_locals.alloc(local("right", ty));
        let equals = h.user_fn_full(
            name,
            Vec::new(),
            vec![param("left", ty, left), param("right", ty, right)],
            h.boolean,
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
        let subject = locals.alloc(local("subject", ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(subject, ty),
                    vec![arm(
                        hir::Pattern::Literal {
                            value: literal,
                            equality: hir::LiteralPatternEquality::Ordinary {
                                equals: hir::Callable::Function(equals),
                            },
                            subject_ty: ty,
                        },
                        None,
                        Vec::new(),
                    )],
                    hir::WhenFallback::Else(Vec::new()),
                )],
            },
        );
        (lower(&h.finish(main)), name)
    }

    for string in [false, true] {
        let (module, expected_name) = lower_case(string);
        let body = &module.functions[module.entry].body;
        let (call, result) = entry_statements(body)
            .iter()
            .find_map(|statement| {
                matches!(statement.kind, mir::StatementKind::Call(_))
                    .then(|| statement_call(statement))
            })
            .expect("ordinary literal pattern calls its HIR-selected equality target");
        assert!(matches!(call.target.kind, mir::CallKind::Direct));
        let mir::Callee::User(equals) = call.target.callee else {
            panic!("the ordinary equality target is a user function");
        };
        assert_eq!(module.functions[equals].name, expected_name);
        assert!(matches!(call.args.as_slice(), [scrutinee, _]
            if matches!(scrutinee.kind, mir::ExprKind::Local(_))));
        let result = result.expect("equals returns Boolean");
        let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
            panic!("literal equality result controls the pattern branch")
        };
        assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == result));
    }
}

#[test]
fn integer_literal_patterns_lower_to_exact_typed_comparisons() {
    for source_kind in hir::IntegerKind::ALL {
        let mut h = Harness::new();
        let ty = h.integer(source_kind);
        let literal = integer_lit(&h, source_kind, 1);
        let equality_expr = integer_binary(
            &mut h,
            source_kind,
            hir::NoGcIntegerOperation::Equals,
            literal.clone(),
            literal.clone(),
        );
        let hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc { target, .. },
            ..
        } = equality_expr.kind
        else {
            panic!("test harness constructs typed integer equals");
        };
        let mut locals = Arena::new();
        let subject = locals.alloc(local("subject", ty));
        let main = h.user_fn(
            "main",
            hir::Body {
                locals,
                statements: vec![when_stmt(
                    local_ref(subject, ty),
                    vec![arm(
                        hir::Pattern::Literal {
                            value: literal,
                            equality: hir::LiteralPatternEquality::Integer {
                                kind: source_kind,
                                target,
                            },
                            subject_ty: ty,
                        },
                        None,
                        Vec::new(),
                    )],
                    hir::WhenFallback::Else(Vec::new()),
                )],
            },
        );
        let module = lower(&h.finish(main));
        let body = &module.functions[module.entry].body;
        assert!(
            entry_statements(body)
                .iter()
                .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_))),
            "integer pattern equality is not lowered as a function call",
        );
        let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
            panic!("a refutable integer literal branches");
        };
        let mir::ExprKind::IntegerCompare {
            operation,
            lhs,
            rhs,
        } = &cond.kind
        else {
            panic!("integer literal pattern lowers directly to IntegerCompare");
        };
        let expected_kind = lower_integer_kind(source_kind);
        assert_eq!(operation.operand_kind(), expected_kind);
        assert_eq!(operation.operator(), mir::IntegerComparisonOperator::Equal);
        assert!(matches!(lhs.kind, mir::ExprKind::Local(_)));
        assert!(matches!(
            rhs.kind,
            mir::ExprKind::IntegerLiteral(value) if value.kind() == expected_kind
        ));
    }
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
Module mangling=compact-v2
  struct Point (x: Int, y: Int)
  fun main @scoop_main() -> Unit
    bb0 entry
      val $bind.1: (Int, String)
        Type (Int, String)
        TupleLiteral
          Type Int
          IntegerLiteral Int value=1 bits=0x00000001
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
      call p: Point = @scoop.ctor.Point.$c0 direct
        Type Int
        IntegerLiteral Int value=3 bits=0x00000003
        Type Int
        IntegerLiteral Int value=4 bits=0x00000004
      val $bind.2: Point
        Type Point
        Local p
      val x: Int
        Type Int
        FieldAccess 0
          Type Point
          Local $bind.2
      return
  fun ctor.Point.$c0 @scoop.ctor.Point.$c0(x: Int, y: Int) -> Point <no-gc>
    bb0 entry
      return
        Type Point
        StructInit Point
          Type Int
          Local x
          Type Int
          Local y
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}
