use super::*;

#[test]
fn try_and_throw_become_explicit_cfg() {
    // try { throw MyError() } catch (e: MyError) { 1 } finally { 2 }
    // — MIR keeps the structured form (DESIGN 3.3); the
    // control-flow expansion is LIR's job.
    let mut h = Harness::new();
    let my_error = h.exception("MyError");
    let error_ty = h.class_ty(my_error);
    let error_constructor = h
        .class_constructor_applications
        .iter()
        .find_map(|(id, app)| {
            (app.constructor == h.classes[my_error].constructors[0]).then_some(id)
        })
        .expect("exception primary constructor application");
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", error_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Throw(expr(
                    hir::ExprKind::ClassInit {
                        constructor: error_constructor,
                        args: Vec::new(),
                    },
                    error_ty,
                )))],
                catches: vec![hir::CatchClause {
                    local: e,
                    ty: error_ty,
                    body: vec![expr_stmt(int_lit(&h, 1))],
                    span: SPAN,
                }],
                finally_body: Some(vec![expr_stmt(int_lit(&h, 2))]),
            }))],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module mangling=compact-v2
  class MyError vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      goto bb8
    bb1 try.unwind.1
      landing_pad cleanup=false
      goto bb2
    bb2 try.dispatch.2
      begin_catch
      branch bb9 bb10
        Type Boolean
        IsInstance MyError
          Type Any
          CaughtException
    bb3 try.handler_pad.3
      landing_pad cleanup=true
      goto bb4
    bb4 try.handler_cleanup.4
      end_catch
      Type Int
      IntegerLiteral Int value=2 bits=0x00000002
      resume
    bb5 try.exit_pad.5
      landing_pad cleanup=true
      goto bb6
    bb6 try.exit_cleanup.6
      end_catch
      resume
    bb7 try.end.7
      return
    bb8 try.body.8 unwind bb1
      assign $new.1
        Type MyError
        ClassAlloc MyError
      call @scoop.init.MyError.$c0 direct
        Type MyError
        Local $new.1
      throw unwind bb1
        Type MyError
        Local $new.1
    bb9 try.catch.9
      val e: MyError
        Type MyError
        Retype MyError
          Type Any
          CaughtException
      goto bb11
    bb10 try.next.10 unwind bb5
      Type Int
      IntegerLiteral Int value=2 bits=0x00000002
      rethrow unwind bb5
    bb11 scope.11 unwind bb3
      Type Int
      IntegerLiteral Int value=1 bits=0x00000001
      goto bb12
    bb12 scope.12
      end_catch
      Type Int
      IntegerLiteral Int value=2 bits=0x00000002
      goto bb7
  fun init.MyError.$c0 @scoop.init.MyError.$c0(this: MyError) -> Unit
    bb0 entry
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn typed_and_builtin_unary_operators_map_to_mir_unops() {
    let mut h = Harness::new();
    let one = int_lit(&h, 1);
    let negation = integer_unary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::NoGcIntegerOperation::UnaryMinus,
        one,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(negation),
                expr_stmt(expr(
                    hir::ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(bool_lit(&h, true)),
                    },
                    h.boolean,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let [integer, boolean] = entry_statements(body) else {
        panic!("expected the two unary expression statements")
    };
    assert!(matches!(
        integer.kind,
        mir::StatementKind::Expr(mir::Expr {
            kind: mir::ExprKind::IntegerUnary { operation, .. },
            ..
        }) if operation.operator() == mir::IntegerUnaryOperator::Negate
    ));
    assert!(matches!(
        boolean.kind,
        mir::StatementKind::Expr(mir::Expr {
            kind: mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                ..
            },
            ..
        })
    ));
}

#[test]
fn field_access_uses_zero_based_indices() {
    let mut h = Harness::new();
    let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
    let point_ty = h.struct_ty(point);
    let point_application = h.struct_application_of(point_ty);
    let pair = h.tuple(&[h.int, h.string]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let t = locals.alloc(local("t", pair));
    let y = locals.alloc(local("y", h.int));
    let s = locals.alloc(local("s", h.string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                // `p.y`
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(local_ref(p, point_ty)),
                            field: hir::FieldRef::StructField {
                                application: point_application,
                                index: 1,
                            },
                        },
                        h.int,
                    ),
                ),
                // `t._2`
                val_decl(
                    s,
                    expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(local_ref(t, pair)),
                            field: hir::FieldRef::TupleIndex(1),
                        },
                        h.string,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    for statement in entry_statements(body) {
        let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(
            init.kind,
            mir::ExprKind::FieldAccess { index: 1, .. }
        ));
    }
}

#[test]
fn control_flow_becomes_cfg() {
    let mut h = Harness::new();
    let println = h.println_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::If {
                    cond: bool_lit(&h, true),
                    then_body: vec![expr_stmt(call(&h, println, vec![str_lit(&h, "a")]))],
                    else_body: Some(vec![expr_stmt(call(&h, println, vec![str_lit(&h, "b")]))]),
                }),
                stmt(hir::StatementKind::While {
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, false),
                    body: vec![],
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Branch { .. }
    ));
    assert_eq!(block_named(body, "if.then").statements.len(), 1);
    assert_eq!(block_named(body, "if.else").statements.len(), 1);
    assert!(matches!(
        block_named(body, "while.cond").terminator,
        mir::Terminator::Branch { .. }
    ));
    assert!(block_named(body, "while.body").statements.is_empty());
}

#[test]
fn while_condition_setup_runs_before_the_first_check_and_on_the_backedge() {
    let mut h = Harness::new();
    let condition = h.user_fn_full(
        "condition",
        Vec::new(),
        Vec::new(),
        h.boolean,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(bool_lit(&h, false)),
            })],
        },
    );
    let mut locals = Arena::new();
    let condition_value = locals.alloc(local("$condition.0", h.boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::While {
                condition_setup: vec![val_decl(
                    condition_value,
                    call_typed(condition, Vec::new(), h.boolean),
                )],
                cond: local_ref(condition_value, h.boolean),
                body: Vec::new(),
            })],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;
    let header = body
        .blocks
        .iter()
        .find_map(|(id, block)| block.name.starts_with("while.cond").then_some(id))
        .expect("the loop has one explicit header");
    let loop_body = body
        .blocks
        .iter()
        .find_map(|(id, block)| block.name.starts_with("while.body").then_some(id))
        .expect("the loop has one body");
    let condition_local = body
        .locals
        .iter()
        .find_map(|(id, local)| (local.name == "$condition.0").then_some(id))
        .expect("the HIR condition temporary keeps one MIR local identity");

    assert!(entry_statements(body).is_empty());
    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Goto(target) if target == header
    ));
    let [setup] = body.blocks[header].statements.as_slice() else {
        panic!("the condition side effect is emitted once in the loop header")
    };
    let (call, destination) = statement_call(setup);
    let mir::Callee::User(callee) = call.target.callee else {
        panic!("the condition setup keeps its exact user-call target")
    };
    assert_eq!(module.functions[callee].name, "condition");
    assert_eq!(destination, Some(condition_local));
    assert!(!body.locals[condition_local].mutable);
    let mir::Terminator::Branch { cond, .. } = &body.blocks[header].terminator else {
        panic!("the condition is evaluated after its header setup")
    };
    assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == condition_local));
    assert!(body.blocks[loop_body].statements.is_empty());
    assert!(matches!(
        body.blocks[loop_body].terminator,
        mir::Terminator::Goto(target) if target == header
    ));
    assert!(
        body.locals
            .iter()
            .all(|(_, local)| local.name != "$cond" && !local.name.starts_with("$cond.")),
        "header normalization must not synthesize a mutable condition cache"
    );
    let setup_call_count = body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter(|statement| {
            let mir::StatementKind::Call(effect) = &statement.kind else {
                return false;
            };
            let call = match effect {
                mir::CallEffect::Unit(call) => call,
                mir::CallEffect::Value { call, .. } => call,
            };
            matches!(call.target.callee, mir::Callee::User(id) if module.functions[id].name == "condition")
        })
        .count();
    assert_eq!(setup_call_count, 1, "HIR condition setup is lowered once");
}

#[test]
fn terminating_while_header_setup_does_not_reopen_condition_or_exit_flow() {
    let mut h = Harness::new();
    let println = h.println_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    condition_setup: vec![stmt(hir::StatementKind::Return { value: None })],
                    cond: bool_lit(&h, true),
                    body: vec![expr_stmt(call(
                        &h,
                        println,
                        vec![str_lit(&h, "unreachable body")],
                    ))],
                }),
                expr_stmt(call(&h, println, vec![str_lit(&h, "unreachable exit")])),
            ],
        },
    );
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    assert_eq!(
        body.blocks.len(),
        2,
        "a sealed header creates no body or exit"
    );
    assert!(entry_statements(body).is_empty());
    let header = block_named(body, "while.cond");
    assert!(header.statements.is_empty());
    assert!(matches!(
        header.terminator,
        mir::Terminator::Return { value: None }
    ));
    assert!(
        body.blocks.iter().all(|(_, block)| {
            !block.name.starts_with("while.body") && !block.name.starts_with("while.exit")
        }),
        "terminating setup must leave the surrounding CFG sealed"
    );
}

#[test]
fn while_condition_prelude_is_owned_by_the_header_and_reentered_by_the_backedge() {
    let mut h = Harness::new();
    h.exception("UnwrapException");
    let boolean = h.boolean;
    let option_boolean = h.option(boolean);
    let mut locals = Arena::new();
    let option = locals.alloc(local("option", option_boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    option,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(bool_lit(&h, true))),
                        option_boolean,
                    ),
                ),
                stmt(hir::StatementKind::While {
                    condition_setup: Vec::new(),
                    cond: expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(option, option_boolean)),
                            trap_on_none: true,
                        },
                        boolean,
                    ),
                    body: Vec::new(),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let header = body
        .blocks
        .iter()
        .find_map(|(id, block)| block.name.starts_with("while.cond").then_some(id))
        .expect("the loop has one header entry");
    let loop_body = body
        .blocks
        .iter()
        .find_map(|(id, block)| block.name.starts_with("while.body").then_some(id))
        .expect("the loop has one body");

    assert_eq!(entry_statements(body).len(), 1);
    assert!(matches!(
        body.blocks[body.entry].terminator,
        mir::Terminator::Goto(target) if target == header
    ));
    assert!(
        body.blocks[header]
            .statements
            .iter()
            .any(|statement| matches!(
                statement.kind,
                mir::StatementKind::ValDecl { local, .. }
                    if body.locals[local].name.starts_with("$opt.") && !body.locals[local].mutable
            ))
    );
    assert!(matches!(
        body.blocks[header].terminator,
        mir::Terminator::Branch {
            cond: mir::Expr {
                kind: mir::ExprKind::VariantTest { .. },
                ..
            },
            ..
        }
    ));
    assert!(
        body.blocks
            .iter()
            .any(|(_, block)| matches!(block.terminator, mir::Terminator::Throw { .. })),
        "the None path of the header-owned null assertion throws"
    );
    assert!(matches!(
        body.blocks[loop_body].terminator,
        mir::Terminator::Goto(target) if target == header
    ));
}
