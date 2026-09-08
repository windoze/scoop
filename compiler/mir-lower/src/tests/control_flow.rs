use super::*;

fn block_id_named(body: &mir::Body, prefix: &str) -> mir::BlockId {
    body.blocks
        .iter()
        .find_map(|(id, block)| block.name.starts_with(prefix).then_some(id))
        .unwrap_or_else(|| panic!("missing MIR block `{prefix}`"))
}

fn block_contains_int(block: &mir::BasicBlock, value: u32) -> bool {
    block.statements.iter().any(|statement| {
        matches!(
            &statement.kind,
            mir::StatementKind::Expr(mir::Expr {
                kind: mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(bits)),
                ..
            }) if *bits == value
        )
    })
}

fn loop_header_poll_blocks(body: &mir::Body) -> Vec<mir::BlockId> {
    body.loop_header_polls
        .iter()
        .map(|target| target.header())
        .collect()
}

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
      call @scoop.init.C7_MyErrorX.$c0 direct
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
      end_catch
      goto bb12
    bb12 scope.12
      Type Int
      IntegerLiteral Int value=2 bits=0x00000002
      goto bb7
  fun init.MyError.$c0 @scoop.init.C7_MyErrorX.$c0(this: MyError) -> Unit
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
                            field: h.struct_field_ref(point_application, 1),
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
fn control_flow_becomes_cfg_and_marks_the_loop_header_poll() {
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
                    target: hir::LoopId::from_raw(0),
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
    assert_eq!(
        loop_header_poll_blocks(body),
        vec![block_id_named(body, "while.cond")]
    );
    assert!(
        mir::dump(&module)
            .lines()
            .any(|line| { line.contains("while.cond") && line.contains("<loop-header-poll>") })
    );
}

#[test]
fn loop_header_poll_owns_while_condition_setup_and_the_backedge() {
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
                target: hir::LoopId::from_raw(0),
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
    assert_eq!(loop_header_poll_blocks(body), vec![header]);
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
fn terminating_while_still_marks_its_loop_header_poll() {
    let mut h = Harness::new();
    let println = h.println_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target: hir::LoopId::from_raw(0),
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
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;

    assert_eq!(
        body.blocks.len(),
        3,
        "the typed loop keeps its complete exit target"
    );
    assert!(entry_statements(body).is_empty());
    let header_id = block_id_named(body, "while.cond");
    let header = &body.blocks[header_id];
    assert!(header.statements.is_empty());
    assert!(matches!(
        header.terminator,
        mir::Terminator::Return { value: None }
    ));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block.name.starts_with("while.body")),
        "terminating setup must not allocate a body"
    );
    let exit = block_named(body, "while.exit");
    assert!(exit.statements.is_empty());
    assert!(matches!(exit.terminator, mir::Terminator::Unreachable));
    assert_eq!(loop_header_poll_blocks(body), vec![header_id]);
    assert!(
        body.blocks
            .iter()
            .flat_map(|(_, block)| &block.statements)
            .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_))),
        "a return in the setup keeps the detached exit and all successors unreachable"
    );
}

#[test]
fn direct_setup_break_preserves_the_loop_header_poll_and_exit_target() {
    let mut h = Harness::new();
    let target = hir::LoopId::from_raw(0);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: vec![stmt(hir::StatementKind::Break { target })],
                    cond: bool_lit(&h, true),
                    body: vec![expr_stmt(int_lit(&h, 99))],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let header = block_id_named(body, "while.cond");
    let exit = block_id_named(body, "while.exit");

    assert_eq!(loop_header_poll_blocks(body), vec![header]);

    assert!(matches!(
        body.blocks[header].terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block.name.starts_with("while.body")),
        "a direct setup break reaches the exit without allocating the body"
    );
    assert!(block_contains_int(&body.blocks[exit], 7));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block_contains_int(block, 99)),
        "the unreachable loop body is not lowered"
    );
}

#[test]
fn loop_body_transfers_keep_one_loop_header_poll_target_without_merge_fallthrough() {
    let mut h = Harness::new();
    let target = hir::LoopId::from_raw(0);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, true),
                    body: vec![
                        stmt(hir::StatementKind::If {
                            cond: bool_lit(&h, true),
                            then_body: vec![stmt(hir::StatementKind::Continue { target })],
                            else_body: Some(vec![stmt(hir::StatementKind::Break { target })]),
                        }),
                        expr_stmt(int_lit(&h, 99)),
                    ],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let header = block_id_named(body, "while.cond");
    let exit = block_id_named(body, "while.exit");
    let then_block = block_named(body, "if.then");
    let else_block = block_named(body, "if.else");
    let merge = block_named(body, "if.merge");

    assert_eq!(loop_header_poll_blocks(body), vec![header]);

    assert!(matches!(
        then_block.terminator,
        mir::Terminator::Goto(target) if target == header
    ));
    assert!(matches!(
        else_block.terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(matches!(merge.terminator, mir::Terminator::Unreachable));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block_contains_int(block, 99)),
        "two abrupt branches must not manufacture a reachable merge"
    );
    assert!(block_contains_int(&body.blocks[exit], 7));
}

#[test]
fn outer_break_runs_finally_before_reaching_the_loop_exit() {
    let mut h = Harness::new();
    let target = hir::LoopId::from_raw(0);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, true),
                    body: vec![
                        stmt(hir::StatementKind::Try(hir::Try {
                            body: vec![stmt(hir::StatementKind::Break { target })],
                            catches: Vec::new(),
                            finally_body: Some(vec![expr_stmt(int_lit(&h, 41))]),
                        })),
                        expr_stmt(int_lit(&h, 99)),
                    ],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let exit = block_id_named(body, "while.exit");
    let try_body = block_named(body, "try.body");

    assert!(block_contains_int(try_body, 41));
    assert!(matches!(
        try_body.terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(block_contains_int(&body.blocks[exit], 7));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block_contains_int(block, 99)),
        "the pending break seals the remainder of the loop body"
    );
}

#[test]
fn non_unit_return_is_saved_before_finally_and_resumed_after_it() {
    let mut h = Harness::new();
    let int = h.int;
    h.user_fn_full(
        "value",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Return {
                    value: Some(int_lit(&h, 9)),
                })],
                catches: Vec::new(),
                finally_body: Some(vec![expr_stmt(int_lit(&h, 41))]),
            }))],
        },
    );
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "value").then_some(&function.body))
        .expect("the non-Unit test function is lowered");
    let return_slot = body
        .locals
        .iter()
        .find_map(|(id, local)| local.name.starts_with("$return.").then_some(id))
        .expect("a return crossing finally has stable storage");
    let try_body = block_named(body, "try.body");
    assert!(try_body.statements.iter().any(|statement| {
        matches!(
            &statement.kind,
            mir::StatementKind::ValDecl { local, init }
                if *local == return_slot
                    && matches!(
                        init.kind,
                        mir::ExprKind::IntegerLiteral(
                            mir::MirIntegerConstant::Signed32(9)
                        )
                    )
        )
    }));
    let mir::Terminator::Goto(finally_block) = &try_body.terminator else {
        panic!("the saved return enters finally")
    };
    let finally_block = &body.blocks[*finally_block];
    assert!(block_contains_int(finally_block, 41));
    assert!(matches!(
        &finally_block.terminator,
        mir::Terminator::Return {
            value: Some(mir::Expr {
                kind: mir::ExprKind::Local(local),
                ..
            })
        } if *local == return_slot
    ));
}

#[test]
fn generic_unit_return_runs_finally_before_bare_return() {
    let mut h = Harness::new();
    let unit = h.unit;
    let producer = identity_fn(&mut h, "produce");
    let type_param = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let producer_for_type_param = h.instantiate(producer, vec![type_param]);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", type_param));
    let function = h.user_fn_full(
        "throughFinally",
        vec!["T".to_string()],
        vec![param("value", type_param, value)],
        type_param,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Try(hir::Try {
                body: vec![stmt(hir::StatementKind::Return {
                    value: Some(generic_call(
                        producer_for_type_param,
                        vec![local_ref(value, type_param)],
                        type_param,
                    )),
                })],
                catches: Vec::new(),
                finally_body: Some(vec![expr_stmt(int_lit(&h, 41))]),
            }))],
        },
    );
    let unit_instance = h.instantiate(function, vec![unit]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(
                unit_instance,
                vec![expr(hir::ExprKind::UnitLiteral, unit)],
                unit,
            ))],
        },
    );

    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == "throughFinally" && function.return_ty == mir::Type::Unit)
                .then_some(function)
        })
        .expect("throughFinally<Unit> is lowered");
    assert!(
        function
            .body
            .locals
            .iter()
            .all(|(_, local)| !local.name.starts_with("$return.")),
        "a Unit return needs no payload slot"
    );

    let try_body = block_named(&function.body, "try.body");
    let call_statement = try_body
        .statements
        .iter()
        .find(|statement| matches!(&statement.kind, mir::StatementKind::Call(_)))
        .expect("the Unit-valued generic producer call is evaluated");
    let (call, destination) = statement_call(call_statement);
    assert!(
        destination.is_none(),
        "the specialized producer returns Unit"
    );
    let producer = module
        .functions
        .iter()
        .find_map(|(id, function)| {
            (function.name == "produce" && function.return_ty == mir::Type::Unit).then_some(id)
        })
        .expect("produce<Unit> is lowered");
    assert_eq!(
        call.target.callee,
        mir::Callee::Monomorphized(instance_id(&module, producer))
    );
    assert!(
        try_body.unwind.is_some(),
        "a producer failure still enters the exceptional finally path"
    );
    let mir::Terminator::Goto(finally_block) = &try_body.terminator else {
        panic!("the Unit return enters finally after evaluating its payload")
    };
    let finally_block = &function.body.blocks[*finally_block];
    assert!(block_contains_int(finally_block, 41));
    assert!(matches!(
        &finally_block.terminator,
        mir::Terminator::Return { value: None }
    ));
}

#[test]
fn catch_break_ends_the_catch_once_then_runs_finally_before_loop_exit() {
    let mut h = Harness::new();
    let error = h.exception("MyError");
    let error_ty = h.class_ty(error);
    let target = hir::LoopId::from_raw(0);
    let inner = hir::LoopId::from_raw(1);
    let mut locals = Arena::new();
    let caught = locals.alloc(local("caught", error_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, true),
                    body: vec![stmt(hir::StatementKind::Try(hir::Try {
                        body: Vec::new(),
                        catches: vec![hir::CatchClause {
                            local: caught,
                            ty: error_ty,
                            body: vec![
                                stmt(hir::StatementKind::While {
                                    target: inner,
                                    condition_setup: Vec::new(),
                                    cond: bool_lit(&h, true),
                                    body: vec![stmt(hir::StatementKind::Break { target: inner })],
                                }),
                                expr_stmt(int_lit(&h, 43)),
                                stmt(hir::StatementKind::Break { target }),
                            ],
                            span: SPAN,
                        }],
                        finally_body: Some(vec![expr_stmt(int_lit(&h, 42))]),
                    }))],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let exit = block_id_named(body, "while.exit");
    let catch = block_named(body, "try.catch");
    let mir::Terminator::Goto(catch_body) = &catch.terminator else {
        panic!("the catch binding enters its handler unwind context")
    };
    let catch_body = &body.blocks[*catch_body];
    let mir::Terminator::Goto(inner_header) = &catch_body.terminator else {
        panic!("the catch body enters its nested loop")
    };
    let mir::Terminator::Branch {
        then_block: inner_body,
        ..
    } = &body.blocks[*inner_header].terminator
    else {
        panic!("the nested loop has a conditional header")
    };
    let mir::Terminator::Goto(inner_exit) = &body.blocks[*inner_body].terminator else {
        panic!("the nested break targets its own exit")
    };
    let inner_exit = &body.blocks[*inner_exit];
    assert!(
        inner_exit.statements.first().is_some_and(|statement| {
            matches!(
                &statement.kind,
                mir::StatementKind::Expr(mir::Expr {
                    kind: mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(43)),
                    ..
                })
            )
        }),
        "a break whose target remains inside the catch runs no catch cleanup"
    );
    let end_catch_count = inner_exit
        .statements
        .iter()
        .filter(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Eh(mir::EhStatement::EndCatch)
            )
        })
        .count();
    assert_eq!(end_catch_count, 1, "the normal catch exit is balanced once");
    let mir::Terminator::Goto(finally_block) = &inner_exit.terminator else {
        panic!("the catch cleanup continues in the enclosing unwind context")
    };
    let finally_block = &body.blocks[*finally_block];
    assert!(block_contains_int(finally_block, 42));
    assert!(matches!(
        finally_block.terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(block_contains_int(&body.blocks[exit], 7));
}

#[test]
fn transfer_from_cleanup_free_try_nested_in_catch_reaches_outer_cleanup() {
    let mut h = Harness::new();
    let error = h.exception("MyError");
    let error_ty = h.class_ty(error);
    let target = hir::LoopId::from_raw(0);
    let mut locals = Arena::new();
    let outer_caught = locals.alloc(local("outerCaught", error_ty));
    let inner_caught = locals.alloc(local("innerCaught", error_ty));
    let escaping_try = hir::Try {
        body: vec![stmt(hir::StatementKind::Break { target })],
        catches: vec![hir::CatchClause {
            local: inner_caught,
            ty: error_ty,
            body: vec![stmt(hir::StatementKind::Break { target })],
            span: SPAN,
        }],
        finally_body: None,
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, true),
                    body: vec![stmt(hir::StatementKind::Try(hir::Try {
                        body: Vec::new(),
                        catches: vec![hir::CatchClause {
                            local: outer_caught,
                            ty: error_ty,
                            body: vec![stmt(hir::StatementKind::Try(escaping_try))],
                            span: SPAN,
                        }],
                        finally_body: Some(vec![expr_stmt(int_lit(&h, 42))]),
                    }))],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let exit = block_id_named(body, "while.exit");
    let normal_outer_catch_exit = body
        .blocks
        .iter()
        .find_map(|(_, block)| {
            if !block.name.starts_with("try.body")
                || !block.statements.iter().any(|statement| {
                    matches!(
                        statement.kind,
                        mir::StatementKind::Eh(mir::EhStatement::EndCatch)
                    )
                })
            {
                return None;
            }
            let mir::Terminator::Goto(next) = &block.terminator else {
                return None;
            };
            block_contains_int(&body.blocks[*next], 42).then_some(*next)
        })
        .expect("the nested cleanup-free try reaches its outer catch cleanup");
    assert!(matches!(
        body.blocks[normal_outer_catch_exit].terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(block_contains_int(&body.blocks[exit], 7));
}

#[test]
fn finally_return_overrides_a_pending_setup_break_without_opening_the_exit() {
    let mut h = Harness::new();
    let target = hir::LoopId::from_raw(0);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: vec![stmt(hir::StatementKind::Try(hir::Try {
                        body: vec![stmt(hir::StatementKind::Break { target })],
                        catches: Vec::new(),
                        finally_body: Some(vec![stmt(hir::StatementKind::Return { value: None })]),
                    }))],
                    cond: bool_lit(&h, true),
                    body: vec![expr_stmt(int_lit(&h, 99))],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let exit = block_id_named(body, "while.exit");

    assert!(matches!(
        body.blocks[exit].terminator,
        mir::Terminator::Unreachable
    ));
    assert!(
        body.blocks.iter().all(|(_, block)| {
            !matches!(block.terminator, mir::Terminator::Goto(target) if target == exit)
        }),
        "an overriding finally return discards the pending break edge"
    );
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block.name.starts_with("while.body")),
        "an overridden setup break must not reopen condition or body flow"
    );
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| { !block_contains_int(block, 99) && !block_contains_int(block, 7) }),
        "the finally return seals every statement after the setup"
    );
}

#[test]
fn nested_finally_loops_mark_every_loop_header_poll_target() {
    let mut h = Harness::new();
    let outer = hir::LoopId::from_raw(0);
    let inner = hir::LoopId::from_raw(1);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                stmt(hir::StatementKind::While {
                    target: outer,
                    condition_setup: vec![stmt(hir::StatementKind::Try(hir::Try {
                        body: vec![stmt(hir::StatementKind::Break { target: outer })],
                        catches: Vec::new(),
                        finally_body: Some(vec![stmt(hir::StatementKind::While {
                            target: inner,
                            condition_setup: Vec::new(),
                            cond: bool_lit(&h, true),
                            body: vec![stmt(hir::StatementKind::Break { target: inner })],
                        })]),
                    }))],
                    cond: bool_lit(&h, true),
                    body: vec![expr_stmt(int_lit(&h, 99))],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let outer_exit = block_id_named(body, "while.exit");
    let inner_body = block_id_named(body, "while.body");
    let concrete_headers = body
        .blocks
        .iter()
        .filter_map(|(id, block)| block.name.starts_with("while.cond").then_some(id))
        .collect::<Vec<_>>();
    assert_eq!(body.loop_header_polls.len(), concrete_headers.len());
    assert!(
        body.loop_header_polls
            .iter()
            .all(|target| concrete_headers.contains(&target.header()))
    );
    let mir::Terminator::Goto(inner_exit) = &body.blocks[inner_body].terminator else {
        panic!("the inner break targets its own exit")
    };
    assert_ne!(*inner_exit, outer_exit);
    assert!(matches!(
        body.blocks[*inner_exit].terminator,
        mir::Terminator::Goto(target) if target == outer_exit
    ));
    assert!(block_contains_int(&body.blocks[outer_exit], 7));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block_contains_int(block, 99)),
        "the resumed outer break skips the outer loop body"
    );
}

#[test]
fn pattern_decision_with_two_abrupt_arms_keeps_its_merge_unreachable() {
    let mut h = Harness::new();
    let boolean = h.boolean;
    let option_boolean = h.option(boolean);
    let option_application = h.enum_application_of(option_boolean);
    let target = hir::LoopId::from_raw(0);
    let mut locals = Arena::new();
    let subject = locals.alloc(local("subject", option_boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    subject,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(bool_lit(&h, true))),
                        option_boolean,
                    ),
                ),
                stmt(hir::StatementKind::While {
                    target,
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, true),
                    body: vec![
                        stmt(hir::StatementKind::When(hir::When {
                            subject: local_ref(subject, option_boolean),
                            arms: vec![hir::WhenArm {
                                pattern: hir::Pattern::Variant {
                                    application: option_application,
                                    variant: 0,
                                    fields: vec![(0, hir::Pattern::Wildcard)],
                                },
                                guard: None,
                                body: vec![stmt(hir::StatementKind::Continue { target })],
                                span: SPAN,
                            }],
                            fallback: hir::WhenFallback::Else(vec![stmt(
                                hir::StatementKind::Break { target },
                            )]),
                        })),
                        expr_stmt(int_lit(&h, 99)),
                    ],
                }),
                expr_stmt(int_lit(&h, 7)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let body = &module.functions[module.entry].body;
    let header = block_id_named(body, "while.cond");
    let exit = block_id_named(body, "while.exit");
    let pass = block_named(body, "pattern.pass");
    let fallback = block_named(body, "pattern.else");
    let merge = block_named(body, "pattern.merge");

    assert!(matches!(
        pass.terminator,
        mir::Terminator::Goto(target) if target == header
    ));
    assert!(matches!(
        fallback.terminator,
        mir::Terminator::Goto(target) if target == exit
    ));
    assert!(matches!(merge.terminator, mir::Terminator::Unreachable));
    assert!(
        body.blocks
            .iter()
            .all(|(_, block)| !block_contains_int(block, 99)),
        "two abrupt pattern outcomes must not manufacture fallthrough"
    );
    assert!(block_contains_int(&body.blocks[exit], 7));
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
                    target: hir::LoopId::from_raw(0),
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
