use super::*;

#[test]
fn typed_string_concat_lowers_to_its_runtime_target() {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", h.string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                s,
                primitive_binary(
                    hir::PrimitiveBinaryKind::StringConcat,
                    str_lit(&h, "a"),
                    str_lit(&h, "b"),
                    h.string,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let (call, destination) = statement_call(&entry_statements(body)[0]);
    assert_eq!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::StringConcat)
    );
    let destination = destination.expect("String concatenation returns String");
    assert_eq!(body.locals[destination].name, "s");
    assert!(matches!(call.args.as_slice(), [left, right]
            if matches!(left.kind, mir::ExprKind::StringConst(_))
                && matches!(right.kind, mir::ExprKind::StringConst(_))));
}

#[test]
fn typed_primitive_intrinsics_map_to_primitive_mir_ops() {
    let mut h = Harness::new();
    let uint = h.uint();
    let mut statements = Vec::new();
    // Division is not here: its divisor check (M8) makes it a
    // statement sequence — see
    // `division_by_zero_throws_arithmetic_exception`.
    let cases = [
        (
            hir::PrimitiveBinaryKind::IntAdd,
            mir::BinOp::IntAdd,
            h.int,
            h.int,
        ),
        (
            hir::PrimitiveBinaryKind::IntSub,
            mir::BinOp::IntSub,
            h.int,
            h.int,
        ),
        (
            hir::PrimitiveBinaryKind::IntMul,
            mir::BinOp::IntMul,
            h.int,
            h.int,
        ),
        (
            hir::PrimitiveBinaryKind::IntCompareTo,
            mir::BinOp::IntCompareTo,
            h.int,
            h.int,
        ),
        (
            hir::PrimitiveBinaryKind::UIntAdd,
            mir::BinOp::IntAdd,
            uint,
            uint,
        ),
        (
            hir::PrimitiveBinaryKind::UIntSub,
            mir::BinOp::IntSub,
            uint,
            uint,
        ),
        (
            hir::PrimitiveBinaryKind::UIntMul,
            mir::BinOp::IntMul,
            uint,
            uint,
        ),
        (
            hir::PrimitiveBinaryKind::UIntCompareTo,
            mir::BinOp::UIntCompareTo,
            uint,
            h.int,
        ),
    ];
    for (kind, _, operand_ty, result_ty) in &cases {
        statements.push(expr_stmt(primitive_binary(
            *kind,
            expr(hir::ExprKind::IntLiteral(1), *operand_ty),
            expr(hir::ExprKind::IntLiteral(2), *operand_ty),
            *result_ty,
        )));
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements,
        },
    );
    let module = lower(&h.finish(main));

    let expected: Vec<mir::BinOp> = cases.iter().map(|(_, mir_op, _, _)| *mir_op).collect();
    let body = &module.functions[module.entry].body;
    let ops: Vec<mir::BinOp> = entry_statements(body)
        .iter()
        .map(|statement| {
            let mir::StatementKind::Expr(expr) = &statement.kind else {
                panic!("expected an expression statement")
            };
            let mir::ExprKind::Binary { op, .. } = &expr.kind else {
                panic!("expected a binary expression")
            };
            *op
        })
        .collect();
    assert_eq!(ops, expected);
}

#[test]
fn typed_unary_and_string_compare_intrinsics_lower_without_name_lookup() {
    let mut h = Harness::new();
    let uint = h.uint();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(primitive_unary(
                    hir::PrimitiveUnaryKind::IntInc,
                    int_lit(&h, 1),
                    h.int,
                )),
                expr_stmt(primitive_unary(
                    hir::PrimitiveUnaryKind::UIntDec,
                    expr(hir::ExprKind::IntLiteral(2), uint),
                    uint,
                )),
                expr_stmt(primitive_unary(
                    hir::PrimitiveUnaryKind::BooleanNot,
                    bool_lit(&h, true),
                    h.boolean,
                )),
                expr_stmt(primitive_binary(
                    hir::PrimitiveBinaryKind::StringCompareTo,
                    str_lit(&h, "a"),
                    str_lit(&h, "b"),
                    h.int,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let statements = entry_statements(&module.functions[module.entry].body);
    for (statement, expected) in
        statements[..3]
            .iter()
            .zip([mir::BinOp::IntAdd, mir::BinOp::IntSub, mir::BinOp::BoolNe])
    {
        let mir::StatementKind::Expr(expression) = &statement.kind else {
            panic!("typed unary intrinsic must stay an expression")
        };
        match (&expression.kind, expected) {
            (mir::ExprKind::Binary { op, .. }, expected) => assert_eq!(*op, expected),
            (
                mir::ExprKind::Unary {
                    op: mir::UnOp::BoolNot,
                    ..
                },
                mir::BinOp::BoolNe,
            ) => {}
            (actual, _) => panic!("unexpected unary lowering {actual:?}"),
        }
    }
    let (compare, destination) = statement_call(&statements[3]);
    assert_eq!(
        compare.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::StringCompare)
    );
    assert!(destination.is_some());
}

#[test]
fn short_circuit_rhs_calls_stay_on_rhs_edges() {
    let mut h = Harness::new();
    let boolean = h.boolean;
    let rhs = h.user_fn_full(
        "rhs",
        Vec::new(),
        Vec::new(),
        boolean,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(bool_lit(&h, true)),
            })],
        },
    );
    let mut locals = Arena::new();
    let and_result = locals.alloc(local("and_result", boolean));
    let or_result = locals.alloc(local("or_result", boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    and_result,
                    binary(
                        hir::BinOp::And,
                        bool_lit(&h, false),
                        call_typed(rhs, Vec::new(), boolean),
                        boolean,
                    ),
                ),
                val_decl(
                    or_result,
                    binary(
                        hir::BinOp::Or,
                        bool_lit(&h, true),
                        call_typed(rhs, Vec::new(), boolean),
                        boolean,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    let rhs_blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|(_, block)| block)
        .filter(|block| block.name.starts_with("logic.rhs"))
        .collect();
    assert_eq!(rhs_blocks.len(), 2);
    for block in rhs_blocks {
        let (call, destination) = statement_call(&block.statements[0]);
        assert_eq!(call.target.callee, mir::Callee::User(module.top_level[0]));
        assert!(destination.is_some());
    }
    assert!(body.blocks.iter().all(|(_, block)| {
        block.name.starts_with("logic.rhs")
            || block
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
    }));

    let mir::Terminator::Branch {
        then_block,
        else_block,
        ..
    } = body.blocks[body.entry].terminator
    else {
        panic!("`&&` must branch to its RHS or short-circuit block")
    };
    assert!(body.blocks[then_block].name.starts_with("logic.rhs"));
    assert!(body.blocks[else_block].name.starts_with("logic.short"));
}

#[test]
fn nested_calls_are_normalized_left_to_right() {
    let mut h = Harness::new();
    let int = h.int;
    let first_result = int_lit(&h, 1);
    let first = h.user_fn_full(
        "first",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(first_result),
            })],
        },
    );
    let second_result = int_lit(&h, 2);
    let second = h.user_fn_full(
        "second",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(second_result),
            })],
        },
    );
    let mut outer_locals = Arena::new();
    let a = outer_locals.alloc(local("a", int));
    let b = outer_locals.alloc(local("b", int));
    let outer = h.user_fn_full(
        "outer",
        Vec::new(),
        vec![param("a", int, a), param("b", int, b)],
        int,
        hir::Body {
            locals: outer_locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(local_ref(a, int)),
            })],
        },
    );
    let mut locals = Arena::new();
    let result = locals.alloc(local("result", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                result,
                call_typed(
                    outer,
                    vec![
                        call_typed(first, Vec::new(), int),
                        call_typed(second, Vec::new(), int),
                    ],
                    int,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let statements = entry_statements(body);
    assert_eq!(statements.len(), 3);

    let (first_call, first_destination) = statement_call(&statements[0]);
    let first_destination = first_destination.expect("first returns Int");
    assert_eq!(
        first_call.target.callee,
        mir::Callee::User(module.top_level[0])
    );
    let (second_call, second_destination) = statement_call(&statements[1]);
    let second_destination = second_destination.expect("second returns Int");
    assert_eq!(
        second_call.target.callee,
        mir::Callee::User(module.top_level[1])
    );
    let (outer_call, outer_destination) = statement_call(&statements[2]);
    assert_eq!(
        outer_call.target.callee,
        mir::Callee::User(module.top_level[2])
    );
    assert!(matches!(outer_call.args.as_slice(), [first, second]
            if matches!(first.kind, mir::ExprKind::Local(local) if local == first_destination)
                && matches!(second.kind, mir::ExprKind::Local(local) if local == second_destination)));
    let outer_destination = outer_destination.expect("outer returns Int");
    assert_eq!(body.locals[outer_destination].name, "result");
}

#[test]
fn division_by_zero_throws_arithmetic_exception() {
    // val q = 10 / 2 — M8: both operands are evaluated once into
    // hidden locals (left to right), the zero check precedes the
    // division, and a zero divisor throws `ArithmeticException`.
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let int = h.int;
    let mut locals = Arena::new();
    let q = locals.alloc(local("q", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                q,
                primitive_binary(
                    hir::PrimitiveBinaryKind::IntDiv,
                    int_lit(&h, 10),
                    int_lit(&h, 2),
                    int,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  class ArithmeticException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val $div.1: Int
        Type Int
        IntLiteral 10
      val $div.2: Int
        Type Int
        IntLiteral 2
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          Local $div.2
          Type Int
          IntLiteral 0
    bb1 if.then.1
      assign $new.1
        Type ArithmeticException
        ClassAlloc ArithmeticException
      call @scoop.init.ArithmeticException.$c0 direct
        Type ArithmeticException
        Local $new.1
      throw
        Type ArithmeticException
        Local $new.1
    bb2 if.merge.2
      val q: Int
        Type Int
        Binary IntDiv
          Type Int
          Local $div.1
          Type Int
          Local $div.2
      return
  fun init.ArithmeticException.$c0 @scoop.init.ArithmeticException.$c0(this: ArithmeticException) -> Unit
    bb0 entry
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn remainder_uses_the_same_zero_guard_as_division() {
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let int = h.int;
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(primitive_binary(
                hir::PrimitiveBinaryKind::IntRem,
                int_lit(&h, 10),
                int_lit(&h, 3),
                int,
            ))],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    assert!(body.blocks.iter().any(|(_, block)| {
        block.statements.iter().any(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Expr(mir::Expr {
                    kind: mir::ExprKind::Binary {
                        op: mir::BinOp::IntRem,
                        ..
                    },
                    ..
                })
            )
        })
    }));
    assert!(body.blocks.iter().any(|(_, block)| {
        matches!(
            block.terminator,
            mir::Terminator::Branch {
                cond: mir::Expr {
                    kind: mir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        ..
                    },
                    ..
                },
                ..
            }
        )
    }));
}

#[test]
fn uint_division_keeps_unsigned_operation_after_the_zero_guard() {
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let uint = h.uint();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(primitive_binary(
                hir::PrimitiveBinaryKind::UIntDiv,
                expr(hir::ExprKind::IntLiteral(10), uint),
                expr(hir::ExprKind::IntLiteral(2), uint),
                uint,
            ))],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    assert!(body.blocks.iter().any(|(_, block)| {
        block.statements.iter().any(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Expr(mir::Expr {
                    kind: mir::ExprKind::Binary {
                        op: mir::BinOp::UIntDiv,
                        ..
                    },
                    ..
                })
            )
        })
    }));
    assert!(body.blocks.iter().any(|(_, block)| {
        matches!(
            block.terminator,
            mir::Terminator::Branch {
                cond: mir::Expr {
                    kind: mir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        ..
                    },
                    ..
                },
                ..
            }
        )
    }));
}
