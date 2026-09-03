use super::*;

#[test]
fn string_plus_lowers_to_runtime_concat() {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", h.string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                s,
                binary(
                    hir::BinOp::Add,
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
fn primitive_operators_map_to_primitive_mir_ops() {
    let mut h = Harness::new();
    let mut statements = Vec::new();
    // Division is not here: its divisor check (M8) makes it a
    // statement sequence — see
    // `division_by_zero_throws_arithmetic_exception`.
    let int_cases = [
        (hir::BinOp::Add, mir::BinOp::IntAdd),
        (hir::BinOp::Sub, mir::BinOp::IntSub),
        (hir::BinOp::Mul, mir::BinOp::IntMul),
        (hir::BinOp::Lt, mir::BinOp::IntLt),
        (hir::BinOp::Le, mir::BinOp::IntLe),
        (hir::BinOp::Gt, mir::BinOp::IntGt),
        (hir::BinOp::Ge, mir::BinOp::IntGe),
    ];
    for (hir_op, _) in &int_cases {
        let ty = if matches!(hir_op, hir::BinOp::Add | hir::BinOp::Sub | hir::BinOp::Mul) {
            h.int
        } else {
            h.boolean
        };
        statements.push(expr_stmt(binary(
            *hir_op,
            int_lit(&h, 1),
            int_lit(&h, 2),
            ty,
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

    let expected: Vec<mir::BinOp> = int_cases.iter().map(|(_, mir_op)| *mir_op).collect();
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
                binary(hir::BinOp::Div, int_lit(&h, 10), int_lit(&h, 2), int),
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
      call $call.1: ArithmeticException = @scoop.ctor.ArithmeticException direct
      throw
        Type ArithmeticException
        Local $call.1
    bb2 if.merge.2
      val q: Int
        Type Int
        Binary IntDiv
          Type Int
          Local $div.1
          Type Int
          Local $div.2
      return
  fun ctor.ArithmeticException @scoop.ctor.ArithmeticException() -> ArithmeticException
    bb0 entry
      return
        Type ArithmeticException
        ClassInit ArithmeticException
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
            statements: vec![expr_stmt(binary(
                hir::BinOp::Rem,
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
