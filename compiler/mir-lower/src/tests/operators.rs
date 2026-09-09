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
    let mut statements = Vec::new();
    // Division is not here: its divisor check (M8) makes it a
    // statement sequence — see
    // `division_by_zero_throws_arithmetic_exception`.
    let cases = [
        (hir::IntegerKind::SIGNED_32, hir::NoGcIntegerOperation::Add),
        (hir::IntegerKind::SIGNED_32, hir::NoGcIntegerOperation::Sub),
        (hir::IntegerKind::SIGNED_32, hir::NoGcIntegerOperation::Mul),
        (
            hir::IntegerKind::SIGNED_32,
            hir::NoGcIntegerOperation::CompareTo,
        ),
        (
            hir::IntegerKind::UNSIGNED_32,
            hir::NoGcIntegerOperation::Add,
        ),
        (
            hir::IntegerKind::UNSIGNED_32,
            hir::NoGcIntegerOperation::Sub,
        ),
        (
            hir::IntegerKind::UNSIGNED_32,
            hir::NoGcIntegerOperation::Mul,
        ),
        (
            hir::IntegerKind::UNSIGNED_32,
            hir::NoGcIntegerOperation::CompareTo,
        ),
    ];
    for (kind, operation) in cases {
        let lhs = integer_lit(&h, kind, 1);
        let rhs = integer_lit(&h, kind, 2);
        statements.push(expr_stmt(integer_binary(&mut h, kind, operation, lhs, rhs)));
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements,
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let lowered: Vec<_> = entry_statements(body)
        .iter()
        .map(|statement| {
            let mir::StatementKind::Expr(expr) = &statement.kind else {
                panic!("expected an expression statement")
            };
            match &expr.kind {
                mir::ExprKind::IntegerBinary { operation, .. } => {
                    (operation.kind(), operation.operator().name())
                }
                mir::ExprKind::IntegerCompareTo { operation, .. } => {
                    (operation.operand_kind(), "compare-to")
                }
                other => panic!("expected a typed integer expression, got {other:?}"),
            }
        })
        .collect();
    assert_eq!(
        lowered,
        [
            (mir::IntegerKind::SIGNED_32, "add"),
            (mir::IntegerKind::SIGNED_32, "subtract"),
            (mir::IntegerKind::SIGNED_32, "multiply"),
            (mir::IntegerKind::SIGNED_32, "compare-to"),
            (mir::IntegerKind::UNSIGNED_32, "add"),
            (mir::IntegerKind::UNSIGNED_32, "subtract"),
            (mir::IntegerKind::UNSIGNED_32, "multiply"),
            (mir::IntegerKind::UNSIGNED_32, "compare-to"),
        ]
    );
}

#[test]
fn typed_unary_and_string_compare_intrinsics_lower_without_name_lookup() {
    let mut h = Harness::new();
    let int_one = int_lit(&h, 1);
    let int_inc = integer_unary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::NoGcIntegerOperation::Inc,
        int_one,
    );
    let uint_two = integer_lit(&h, hir::IntegerKind::UNSIGNED_32, 2);
    let uint_dec = integer_unary(
        &mut h,
        hir::IntegerKind::UNSIGNED_32,
        hir::NoGcIntegerOperation::Dec,
        uint_two,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(int_inc),
                expr_stmt(uint_dec),
                expr_stmt(primitive_unary(
                    hir::PrimitiveUnaryKind::BooleanNot,
                    bool_lit(&h, true),
                    h.boolean,
                )),
                expr_stmt(primitive_binary(
                    hir::PrimitiveBinaryKind::StringCompareTo,
                    str_lit(&h, "a"),
                    str_lit(&h, "b"),
                    h.long,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let statements = entry_statements(body);
    let unary_ops = statements[..2].iter().map(|statement| {
        let mir::StatementKind::Expr(mir::Expr {
            kind: mir::ExprKind::IntegerUnary { operation, .. },
            ..
        }) = &statement.kind
        else {
            panic!("typed integer unary intrinsic must stay an expression")
        };
        operation.operator()
    });
    assert_eq!(
        unary_ops.collect::<Vec<_>>(),
        [
            mir::IntegerUnaryOperator::Increment,
            mir::IntegerUnaryOperator::Decrement,
        ]
    );
    assert!(matches!(
        statements[2].kind,
        mir::StatementKind::Expr(mir::Expr {
            kind: mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                ..
            },
            ..
        })
    ));
    let (compare, destination) = statement_call(&statements[3]);
    assert_eq!(
        compare.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::StringCompare)
    );
    let destination = destination.expect("the runtime String compare returns Long");
    assert_eq!(
        body.locals[destination].ty,
        mir::Type::Integer(mir::IntegerKind::SIGNED_64)
    );
    assert_eq!(statements.len(), 4);
}

#[test]
fn all_integer_constants_and_conversions_preserve_exact_kinds() {
    let mut h = Harness::new();
    let kinds = hir::IntegerKind::ALL;
    let mut statements = Vec::new();
    for (index, source) in kinds.into_iter().enumerate() {
        let target = kinds[(index + 1) % kinds.len()];
        let operand = integer_lit(&h, source, source.width().raw_mask());
        statements.push(expr_stmt(integer_conversion(
            &mut h, source, target, operand,
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
    let statements = entry_statements(&module.functions[module.entry].body);

    for (index, statement) in statements.iter().enumerate() {
        let source = kinds[index];
        let target = kinds[(index + 1) % kinds.len()];
        let source = crate::types::lower_integer_kind(source);
        let target = crate::types::lower_integer_kind(target);
        let mir::StatementKind::Expr(mir::Expr {
            ty,
            kind:
                mir::ExprKind::IntegerConversion {
                    conversion,
                    operand,
                },
        }) = &statement.kind
        else {
            panic!("integer conversion must lower to its dedicated MIR node")
        };
        assert_eq!(*ty, mir::Type::Integer(target));
        assert_eq!(conversion.source_kind(), source);
        assert_eq!(conversion.target_kind(), target);
        let mir::ExprKind::IntegerLiteral(value) = operand.kind else {
            panic!("conversion operand must retain the exact integer constant")
        };
        assert_eq!(value.kind(), source);
        assert_eq!(value.raw_bits(), source.width().raw_mask());
    }
}

#[test]
fn shifts_mask_long_counts_then_convert_to_the_operand_kind() {
    let mut h = Harness::new();
    let kinds = hir::IntegerKind::ALL;
    let mut statements = Vec::new();
    for kind in kinds {
        let value = integer_lit(&h, kind, 1);
        let count = integer_lit(&h, hir::IntegerKind::SIGNED_64, u64::MAX);
        statements.push(expr_stmt(integer_binary(
            &mut h,
            kind,
            hir::NoGcIntegerOperation::Shl,
            value,
            count,
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
    let statements = entry_statements(&module.functions[module.entry].body);

    for (statement, source_kind) in statements.iter().zip(kinds) {
        let kind = crate::types::lower_integer_kind(source_kind);
        let mir::StatementKind::Expr(mir::Expr {
            kind: mir::ExprKind::IntegerShift {
                operation, count, ..
            },
            ..
        }) = &statement.kind
        else {
            panic!("integer shift must lower to its dedicated MIR node")
        };
        assert_eq!(operation.value_kind(), kind);
        assert_eq!(operation.count_kind(), kind);
        let mir::ExprKind::IntegerConversion {
            conversion,
            operand,
        } = &count.kind
        else {
            panic!("masked Long shift count must be converted to the operand width")
        };
        assert_eq!(conversion.source_kind(), mir::IntegerKind::SIGNED_64);
        assert_eq!(conversion.target_kind(), kind);
        let mir::ExprKind::IntegerBinary {
            operation,
            lhs,
            rhs,
        } = &operand.kind
        else {
            panic!("shift count conversion must consume the masked Long count")
        };
        assert_eq!(operation.kind(), mir::IntegerKind::SIGNED_64);
        assert_eq!(operation.operator(), mir::IntegerBinaryOperator::BitAnd);
        assert_eq!(lhs.ty, mir::Type::Integer(mir::IntegerKind::SIGNED_64));
        let mir::ExprKind::IntegerLiteral(mask) = rhs.kind else {
            panic!("shift mask must be an exact Long constant")
        };
        assert_eq!(mask.kind(), mir::IntegerKind::SIGNED_64);
        assert_eq!(mask.raw_bits(), u64::from(kind.width().bits() - 1));
    }
}

#[test]
fn signed_div_rem_poison_boundaries_are_separated_before_safe_nodes() {
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let signed_kinds = [
        hir::IntegerKind::SIGNED_8,
        hir::IntegerKind::SIGNED_16,
        hir::IntegerKind::SIGNED_32,
        hir::IntegerKind::SIGNED_64,
    ];
    let mut statements = Vec::new();
    for kind in signed_kinds {
        for operator in [hir::IntegerDivRem::Div, hir::IntegerDivRem::Rem] {
            let sign_bit = 1_u64 << (kind.width().bits() - 1);
            let dividend = integer_lit(&h, kind, sign_bit);
            let divisor = integer_lit(&h, kind, kind.width().raw_mask());
            statements.push(expr_stmt(integer_div_rem(
                &mut h, kind, operator, dividend, divisor,
            )));
        }
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements,
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let mut safe = HashMap::new();
    let mut special = HashMap::new();
    let mut zero_guards = 0;

    for (_, block) in body.blocks.iter() {
        if matches!(
            &block.terminator,
            mir::Terminator::Branch {
                cond: mir::Expr {
                    kind: mir::ExprKind::IntegerCompare { operation, rhs, .. },
                    ..
                },
                ..
            } if operation.operator() == mir::IntegerComparisonOperator::Equal
                && matches!(rhs.kind, mir::ExprKind::IntegerLiteral(value) if value.raw_bits() == 0)
        ) {
            zero_guards += 1;
        }
        for statement in &block.statements {
            let mir::StatementKind::Assign { local, value } = &statement.kind else {
                continue;
            };
            if !body.locals[*local].name.starts_with("$div.result") {
                continue;
            }
            match value.kind {
                mir::ExprKind::SafeIntegerDivRem { operation, .. } => {
                    assert!(safe.insert(*local, operation).is_none());
                }
                mir::ExprKind::IntegerLiteral(value) => {
                    assert!(special.insert(*local, value).is_none());
                }
                ref other => panic!("div/rem result assignment has unexpected value {other:?}"),
            }
        }
    }

    assert_eq!(zero_guards, signed_kinds.len() * 2);
    assert_eq!(safe.len(), signed_kinds.len() * 2);
    assert_eq!(special.len(), signed_kinds.len() * 2);
    for (local, operation) in safe {
        let value = special[&local];
        assert_eq!(value.kind(), operation.kind());
        let expected = match operation.operator() {
            mir::SafeIntegerDivRemOperator::Divide => {
                1_u64 << (operation.kind().width().bits() - 1)
            }
            mir::SafeIntegerDivRemOperator::Remainder => 0,
        };
        assert_eq!(value.raw_bits(), expected);
    }
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
    let dividend = int_lit(&h, 10);
    let divisor = int_lit(&h, 2);
    let quotient = integer_div_rem(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::IntegerDivRem::Div,
        dividend,
        divisor,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(q, quotient)],
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
        IntegerLiteral Int value=10 bits=0x0000000a
      val $div.2: Int
        Type Int
        IntegerLiteral Int value=2 bits=0x00000002
      branch bb1 bb2
        Type Boolean
        IntegerCompare equal operands=Int result=Boolean
          Type Int
          Local $div.2
          Type Int
          IntegerLiteral Int value=0 bits=0x00000000
    bb1 if.then.1
      assign $new.1
        Type ArithmeticException
        ClassAlloc ArithmeticException
      call @scoop.init.C19_ArithmeticExceptionX.$c0 direct
        Type ArithmeticException
        Local $new.1
      throw
        Type ArithmeticException
        Local $new.1
    bb2 if.merge.2
      branch bb3 bb4
        Type Boolean
        IntegerCompare equal operands=Int result=Boolean
          Type Int
          Local $div.1
          Type Int
          IntegerLiteral Int value=-2147483648 bits=0x80000000
    bb3 logic.rhs.3
      assign $logic.2
        Type Boolean
        IntegerCompare equal operands=Int result=Boolean
          Type Int
          Local $div.2
          Type Int
          IntegerLiteral Int value=-1 bits=0xffffffff
      goto bb5
    bb4 logic.short.4
      assign $logic.2
        Type Boolean
        BoolLiteral false
      goto bb5
    bb5 logic.merge.5
      branch bb6 bb7
        Type Boolean
        Local $logic.2
    bb6 if.then.6
      assign $div.result.3
        Type Int
        IntegerLiteral Int value=-2147483648 bits=0x80000000
      goto bb8
    bb7 if.else.7
      assign $div.result.3
        Type Int
        SafeIntegerDivRem divide kind=Int
          Type Int
          Local $div.1
          Type Int
          Local $div.2
      goto bb8
    bb8 if.merge.8
      val q: Int
        Type Int
        Local $div.result.3
      return
  fun init.ArithmeticException.$c0 @scoop.init.C19_ArithmeticExceptionX.$c0(this: ArithmeticException) -> Unit
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
    let dividend = int_lit(&h, 10);
    let divisor = int_lit(&h, 3);
    let remainder = integer_div_rem(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::IntegerDivRem::Rem,
        dividend,
        divisor,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(remainder)],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    assert!(body.blocks.iter().any(|(_, block)| {
        block.statements.iter().any(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Assign {
                    value: mir::Expr {
                    kind: mir::ExprKind::SafeIntegerDivRem { operation, .. },
                    ..
                    },
                    ..
                } if operation.operator() == mir::SafeIntegerDivRemOperator::Remainder
            )
        })
    }));
    assert!(body.blocks.iter().any(|(_, block)| {
        matches!(
            block.terminator,
            mir::Terminator::Branch {
                cond: mir::Expr {
                    kind: mir::ExprKind::IntegerCompare { operation, .. },
                    ..
                },
                ..
            } if operation.operator() == mir::IntegerComparisonOperator::Equal
        )
    }));
}

#[test]
fn uint_division_keeps_unsigned_operation_after_the_zero_guard() {
    let mut h = Harness::new();
    h.exception("ArithmeticException");
    let dividend = integer_lit(&h, hir::IntegerKind::UNSIGNED_32, 10);
    let divisor = integer_lit(&h, hir::IntegerKind::UNSIGNED_32, 2);
    let quotient = integer_div_rem(
        &mut h,
        hir::IntegerKind::UNSIGNED_32,
        hir::IntegerDivRem::Div,
        dividend,
        divisor,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(quotient)],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    assert!(body.blocks.iter().any(|(_, block)| {
        block.statements.iter().any(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Expr(mir::Expr {
                    kind: mir::ExprKind::SafeIntegerDivRem { operation, .. },
                    ..
                }) if operation.operator() == mir::SafeIntegerDivRemOperator::Divide
            )
        })
    }));
    assert!(body.blocks.iter().any(|(_, block)| {
        matches!(
            block.terminator,
            mir::Terminator::Branch {
                cond: mir::Expr {
                    kind: mir::ExprKind::IntegerCompare { operation, .. },
                    ..
                },
                ..
            } if operation.operator() == mir::IntegerComparisonOperator::Equal
        )
    }));
}
