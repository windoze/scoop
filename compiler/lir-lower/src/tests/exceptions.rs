use super::*;

/// `try { throw e } catch (e: MyError) { handled() }` minus the
/// throw — the shared shell of the M8 tests: `helper()` in the
/// body, `handled()` in the catch, `cleanup()` in the finally.
fn try_shell(finally: bool) -> (Builder, mir::FunctionId, mir::FunctionId, mir::FunctionId) {
    let mut b = Builder::new();
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let handled = b.user_fn("handled", "scoop.handled", Arena::new(), vec![]);
    let cleanup = if finally {
        b.user_fn("cleanup", "scoop.cleanup", Arena::new(), vec![])
    } else {
        helper
    };
    (b, helper, handled, cleanup)
}

fn my_error(b: &mut Builder) -> mir::ClassId {
    b.class("MyError", None, &[], vec![], vec![])
}

#[test]
fn try_catch_lowers_to_invoke_landingpad_and_rethrow() {
    let (mut b, helper, handled, _) = try_shell(false);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        single_catch_body(
            locals,
            e,
            mir::Type::Class(my_error),
            vec![call_stmt(user_call(helper))],
            None,
            vec![call_stmt(user_call(handled))],
        ),
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.helper() -> void
  block entry
    poll managed-void-target0 sp3 live=[]
    ret
  fun @scoop.handled() -> void
  block entry
    poll managed-void-target0 sp4 live=[]
    ret
  fun @scoop_main() -> void
    local %0 e: ptr<managed>
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    poll managed-void-target2 sp5 live=[]
    br @try.body.8
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    call no-gc-direct-target0 t3 = sig=direct0 (ptr<managed>, ptr<metadata>) -> i1 runtime @scoop_rt_is_instance(local3, td0)
    cbr t3 then @try.catch.9 else @try.next.10
  block try.handler_pad.3
    (t4, t5) = cleanup_pad : (exception_record, ptr<raw>)
    store t4 -> local1
    store t5 -> local2
    br @try.handler_cleanup.4
  block try.handler_cleanup.4
    end_catch
    resume local1
  block try.exit_pad.5
    (t6, t7) = cleanup_pad : (exception_record, ptr<raw>)
    store t6 -> local1
    store t7 -> local2
    br @try.exit_cleanup.6
  block try.exit_cleanup.6
    end_catch
    resume local1
  block try.end.7
    ret
  block try.body.8
    invoke managed-void-target0 sp1 roots=[] sig=void0 () local-fn0() normal @invoke.normal.1 unwind @try.unwind.1
    br @invoke.normal.1
  block try.catch.9
    store local3 -> local0
    invoke managed-void-target1 sp2 roots=[] sig=void1 () local-fn1() normal @invoke.normal.2 unwind @try.handler_pad.3
    br @invoke.normal.2
  block try.next.10
    invoke no-gc-void-target0 sig=void2 () runtime @scoop_rt_rethrow() normal @rethrow.normal.3 unwind @try.exit_pad.5
    br @rethrow.normal.3
  block invoke.normal.1
    t8 = aggregate () : {}
    br @try.end.7
  block invoke.normal.2
    t9 = aggregate () : {}
    end_catch
    br @try.end.7
  block rethrow.normal.3
    unreachable
  td td0 MyError @scoop_td_MyError type-id=2 size=16 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn managed_invoke_roots_have_complete_edge_roles_and_argument_coverage() {
    let mut b = Builder::new();
    let error_class = my_error(&mut b);
    let reference_ty = mir::Type::Class(error_class);

    let mut callee_locals = Arena::new();
    let callee_first = callee_locals.alloc(local("first", reference_ty.clone()));
    let callee_second = callee_locals.alloc(local("second", reference_ty.clone()));
    let callee = b.user_fn_body(
        "callee",
        "scoop.callee",
        vec![
            param("first", reference_ty.clone(), callee_first),
            param("second", reference_ty.clone(), callee_second),
        ],
        mir::Type::Unit,
        body_with_terminator(
            callee_locals,
            Vec::new(),
            mir::Terminator::Return { value: None },
        ),
    );

    let mut locals = Arena::new();
    let live_on_both_edges = locals.alloc(local("both", reference_ty.clone()));
    let argument_only = locals.alloc(local("argumentOnly", reference_ty.clone()));
    let caught = locals.alloc(local("caught", reference_ty.clone()));
    let sink = locals.alloc(local("sink", reference_ty.clone()));
    let call = mir::Call {
        target: mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: mir::Callee::User(callee),
        },
        args: vec![
            local_expr(live_on_both_edges, reference_ty.clone()),
            local_expr(argument_only, reference_ty.clone()),
        ],
    };
    let body = single_catch_body(
        locals,
        caught,
        reference_ty.clone(),
        vec![
            call_stmt(call),
            assign(sink, local_expr(live_on_both_edges, reference_ty.clone())),
        ],
        None,
        vec![assign(
            sink,
            local_expr(live_on_both_edges, reference_ty.clone()),
        )],
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        vec![
            param("both", reference_ty.clone(), live_on_both_edges),
            param("argumentOnly", reference_ty, argument_only),
        ],
        mir::Type::Unit,
        body,
    );
    let module = lower(&b.finish(main));
    let main = module
        .functions
        .iter()
        .find(|function| function.symbol == mir::ENTRY_SYMBOL)
        .expect("main function");
    let roots = main
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.instructions)
        .find_map(|instruction| match instruction {
            lir::Instruction::Invoke {
                site: lir::InvokeSite::Managed(site),
            } => Some(site.roots.as_slice()),
            _ => None,
        })
        .expect("managed invoke");

    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].root.source, lir::CallerRootSource::Param(0));
    assert!(roots[0].normal_live);
    assert!(roots[0].unwind_live);
    assert_eq!(roots[1].root.source, lir::CallerRootSource::Param(1));
    assert!(!roots[1].normal_live);
    assert!(!roots[1].unwind_live);
}

#[test]
fn finally_runs_on_the_normal_catch_and_rethrow_paths() {
    let (mut b, helper, handled, cleanup) = try_shell(true);
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let mut body = single_catch_body(
        locals,
        e,
        mir::Type::Class(my_error),
        vec![call_stmt(user_call(helper))],
        None,
        vec![call_stmt(user_call(handled))],
    );
    let try_body = cfg_block_named(&body, "try.body.8");
    let catch = cfg_block_named(&body, "try.catch.9");
    let next = cfg_block_named(&body, "try.next.10");
    let handler_cleanup = cfg_block_named(&body, "try.handler_cleanup.4");
    let exit_pad = cfg_block_named(&body, "try.exit_pad.5");
    let end = cfg_block_named(&body, "try.end.7");
    let normal_finally = cfg_block(&mut body.blocks, "scope.normal_finally");
    let catch_finally = cfg_block(&mut body.blocks, "scope.catch_finally");
    body.blocks[try_body].terminator = mir::Terminator::Goto(normal_finally);
    set_cfg_block(
        &mut body.blocks,
        normal_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Goto(end),
        None,
    );
    assert!(matches!(
        body.blocks[catch]
            .statements
            .pop()
            .map(|statement| statement.kind),
        Some(mir::StatementKind::Eh(mir::EhStatement::EndCatch))
    ));
    body.blocks[catch].terminator = mir::Terminator::Goto(catch_finally);
    set_cfg_block(
        &mut body.blocks,
        catch_finally,
        vec![
            stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            call_stmt(user_call(cleanup)),
        ],
        mir::Terminator::Goto(end),
        None,
    );
    body.blocks[next].statements = vec![call_stmt(user_call(cleanup))];
    body.blocks[next].unwind = Some(exit_pad);
    body.blocks[handler_cleanup]
        .statements
        .push(call_stmt(user_call(cleanup)));
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));
    let dump = lir::dump(&module);

    // The finally body is inlined on normal completion, after the
    // catch body, on a catch-body exceptional exit, and before the
    // no-match rethrow.
    assert_eq!(dump.matches("local-fn2()").count(), 4);
    // The last copy is on the rethrow path, before the rethrow.
    let rethrow = dump
        .find("runtime @scoop_rt_rethrow()")
        .expect("a rethrow path");
    let last_cleanup = dump
        .rfind("local-fn2()")
        .expect("the rethrow path runs the finally");
    assert!(last_cleanup < rethrow);
    assert!(dump.contains("landingpad"));
    // A caught normal exit ends directly; catch-body exceptions
    // and no-match/rethrow exits have distinct cleanup pads.
    // Exactly one executes on each path.
    assert_eq!(dump.matches("end_catch").count(), 3);
}

#[test]
fn return_inside_try_runs_finally_before_returning() {
    // fun f(): Int { try { return 1 } finally { cleanup() } }
    let (mut b, _, _, cleanup) = try_shell(true);
    let mut locals = Arena::new();
    let result = locals.alloc(local("$return.1", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let unwind = cfg_block(&mut blocks, "try.unwind.1");
    let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
    let exit_pad = cfg_block(&mut blocks, "try.exit_pad.3");
    let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.4");
    let end = cfg_block(&mut blocks, "try.end.5");
    let try_body = cfg_block(&mut blocks, "try.body.6");
    let return_finally = cfg_block(&mut blocks, "scope.7");
    let rethrow_finally = cfg_block(&mut blocks, "scope.8");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(try_body),
        None,
    );
    set_cfg_block(
        &mut blocks,
        unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(dispatch),
        None,
    );
    set_cfg_block(
        &mut blocks,
        dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Goto(rethrow_finally),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        mir::Terminator::Goto(exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Resume,
        None,
    );
    set_cfg_block(
        &mut blocks,
        end,
        Vec::new(),
        mir::Terminator::Unreachable,
        None,
    );
    set_cfg_block(
        &mut blocks,
        try_body,
        vec![val_decl(result, mir::Expr::int(1))],
        mir::Terminator::Goto(return_finally),
        Some(unwind),
    );
    set_cfg_block(
        &mut blocks,
        return_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Return {
            value: Some(local_expr(result, mir::Type::Int)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rethrow_finally,
        vec![call_stmt(user_call(cleanup))],
        mir::Terminator::Rethrow {
            unwind: Some(exit_pad),
        },
        Some(exit_pad),
    );
    let f = b.user_fn_body(
        "f",
        "scoop.f",
        Vec::new(),
        mir::Type::Int,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    // This body cannot enter the unwind path, so final LIR prunes that
    // detached copy together with the dead merge block. The reachable
    // finally copy still runs before the return.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.helper() -> void
  block entry
    poll managed-void-target0 sp3 live=[]
    ret
  fun @scoop.handled() -> void
  block entry
    poll managed-void-target0 sp4 live=[]
    ret
  fun @scoop.cleanup() -> void
  block entry
    poll managed-void-target0 sp5 live=[]
    ret
  fun @scoop.f() -> i64
    local %0 $return.1: i64
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    poll managed-void-target2 sp6 live=[]
    br @try.body.6
  block try.body.6
    store 1 -> local0
    br @scope.7
  block scope.7
    call managed-void-target0 sp1 live=[] sig=void0 () local-fn2()
    t5 = aggregate () : {}
    ret local0
  fun @scoop_main() -> void
  block entry
    poll managed-void-target0 sp7 live=[]
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn throw_outside_try_is_a_throw_instruction() {
    // fun fail(): Unit { throw makeError() } — no try, so the
    // `Throw` instruction ends the block.
    let mut b = Builder::new();
    let my_error = my_error(&mut b);
    let mut ctor_locals = Arena::new();
    let make = b.user_fn_body(
        "makeError",
        "scoop.makeError",
        Vec::new(),
        mir::Type::Class(my_error),
        returning_body(
            std::mem::take(&mut ctor_locals),
            expr(
                mir::Type::Class(my_error),
                mir::ExprKind::ClassInit {
                    class_id: my_error,
                    args: Vec::new(),
                },
            ),
        ),
    );
    let mut main_locals = Arena::new();
    let exception = main_locals.alloc(local("$call.1", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        body_with_terminator(
            main_locals,
            vec![call_value(
                exception,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(make),
                    },
                    args: Vec::new(),
                },
            )],
            mir::Terminator::Throw {
                exception: local_expr(exception, mir::Type::Class(my_error)),
                unwind: None,
            },
        ),
    );
    let module = lower(&b.finish(main));

    // Outside a try the throw is the `Throw` instruction ending
    // the block; the callee stays a plain call.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop.makeError() -> ptr<managed>
  block entry
    poll managed-void-target0 sp3 live=[]
    call managed-direct-target0 sp1 live=[] t0 = sig=direct0 (ptr<metadata>, i64) -> ptr<managed> runtime @scoop_rt_alloc(td0, 16)
    ret t0
  fun @scoop_main() -> void
    local %0 $call.1: ptr<managed>
  block entry
    poll managed-void-target0 sp4 live=[]
    call managed-direct-target0 sp2 live=[] t0 = sig=direct0 () -> ptr<managed> local-fn0()
    store t0 -> local0
    throw local0
    unreachable
  td td0 MyError @scoop_td_MyError type-id=2 size=16 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn throw_inside_try_invokes_to_the_own_landingpad() {
    // try { throw e } catch (e: MyError) {} — the throw must
    // reach this function's own pad, so it is an invoke of the
    // runtime throw entry, not a plain `Throw`.
    let mut b = Builder::new();
    let my_error = my_error(&mut b);
    let mut locals = Arena::new();
    let e = locals.alloc(local("e", mir::Type::Class(my_error)));
    let body = single_catch_body(
        locals,
        e,
        mir::Type::Class(my_error),
        Vec::new(),
        None,
        Vec::new(),
    );
    let try_body = body
        .blocks
        .iter()
        .find_map(|(id, block)| (block.name == "try.body.8").then_some(id))
        .expect("try body block");
    let unwind = body.blocks[try_body].unwind;
    let mut body = body;
    body.blocks[try_body].terminator = mir::Terminator::Throw {
        exception: local_expr(e, mir::Type::Class(my_error)),
        unwind,
    };
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = module
        .functions
        .iter()
        .find(|f| f.symbol == mir::ENTRY_SYMBOL)
        .expect("the entry function");
    // The explicit MIR entry jumps into the try body. That block
    // ends with the invoke; its unwind target starts with the
    // landingpad.
    let entry = function
        .blocks
        .iter()
        .map(|(_, block)| block)
        .find(|block| block.name == "try.body.8")
        .expect("the try body");
    let lir::Instruction::Invoke { site } = entry.instructions.last().expect("the throw invoke")
    else {
        panic!("a throw inside a try must be invoked")
    };
    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop_rt_throw"
    );
    let normal = site.normal();
    let unwind = site.unwind();
    assert!(matches!(
        function.blocks[unwind].instructions.first(),
        Some(lir::Instruction::LandingPad { .. })
    ));
    assert!(matches!(
        function.blocks[normal].terminator,
        lir::Terminator::Unreachable
    ));
    // The invoke block's terminator is the redundant `Br` to the
    // normal target (the codegen convention).
    assert!(matches!(
        entry.terminator,
        lir::Terminator::Br(target) if target == normal
    ));
}

#[test]
fn nested_trys_unwind_to_their_own_pads() {
    // try { try { a() } catch (e1: E1) { b() } } catch (e2: E2) { c() }
    // — `a` unwinds to the inner pad; `b` (in the inner catch)
    // unwinds through the inner cleanup before the outer pad.
    let mut b = Builder::new();
    let e1 = b.class("E1", None, &[], vec![], vec![]);
    let e2 = b.class("E2", None, &[], vec![], vec![]);
    let a = b.user_fn("a", "scoop.a", Arena::new(), vec![]);
    let bb = b.user_fn("b", "scoop.b", Arena::new(), vec![]);
    let c = b.user_fn("c", "scoop.c", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let e1_local = locals.alloc(local("e1", mir::Type::Class(e1)));
    let e2_local = locals.alloc(local("e2", mir::Type::Class(e2)));
    let mut body = single_catch_body(
        locals,
        e2_local,
        mir::Type::Class(e2),
        Vec::new(),
        None,
        vec![call_stmt(user_call(c))],
    );
    let outer_body = cfg_block_named(&body, "try.body.8");
    let outer_unwind = cfg_block_named(&body, "try.unwind.1");
    let outer_dispatch = cfg_block_named(&body, "try.dispatch.2");
    let outer_end = cfg_block_named(&body, "try.end.7");
    let inner_unwind = cfg_block(&mut body.blocks, "try.unwind.inner");
    let inner_dispatch = cfg_block(&mut body.blocks, "try.dispatch.inner");
    let inner_handler_pad = cfg_block(&mut body.blocks, "try.handler_pad.inner");
    let inner_handler_cleanup = cfg_block(&mut body.blocks, "try.handler_cleanup.inner");
    let inner_exit_pad = cfg_block(&mut body.blocks, "try.exit_pad.inner");
    let inner_exit_cleanup = cfg_block(&mut body.blocks, "try.exit_cleanup.inner");
    let inner_end = cfg_block(&mut body.blocks, "try.end.inner");
    let inner_body = cfg_block(&mut body.blocks, "try.body.inner");
    let inner_catch = cfg_block(&mut body.blocks, "try.catch.inner");
    let inner_next = cfg_block(&mut body.blocks, "try.next.inner");
    body.blocks[outer_body].terminator = mir::Terminator::Goto(inner_body);
    set_cfg_block(
        &mut body.blocks,
        inner_unwind,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_dispatch),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_dispatch,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
        mir::Terminator::Branch {
            cond: expr(
                mir::Type::Boolean,
                mir::ExprKind::IsInstance {
                    operand: Box::new(mir::Expr::caught_exception()),
                    check_ty: Box::new(mir::Type::Class(e1)),
                },
            ),
            then_block: inner_catch,
            else_block: inner_next,
        },
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_handler_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_handler_cleanup),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_handler_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Goto(outer_dispatch),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_exit_pad,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: false,
        }))],
        mir::Terminator::Goto(inner_exit_cleanup),
        None,
    );
    set_cfg_block(
        &mut body.blocks,
        inner_exit_cleanup,
        vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
        mir::Terminator::Goto(outer_dispatch),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_end,
        Vec::new(),
        mir::Terminator::Goto(outer_end),
        Some(outer_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_body,
        vec![call_stmt(user_call(a))],
        mir::Terminator::Goto(inner_end),
        Some(inner_unwind),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_catch,
        vec![
            val_decl(
                e1_local,
                expr(
                    mir::Type::Class(e1),
                    mir::ExprKind::Retype {
                        operand: Box::new(mir::Expr::caught_exception()),
                        ty: Box::new(mir::Type::Class(e1)),
                    },
                ),
            ),
            call_stmt(user_call(bb)),
            stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
        ],
        mir::Terminator::Goto(inner_end),
        Some(inner_handler_pad),
    );
    set_cfg_block(
        &mut body.blocks,
        inner_next,
        Vec::new(),
        mir::Terminator::Rethrow {
            unwind: Some(inner_exit_pad),
        },
        None,
    );
    let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = module
        .functions
        .iter()
        .find(|f| f.symbol == mir::ENTRY_SYMBOL)
        .expect("the entry function");
    // The outer primary pad has no incoming exceptional edge after the
    // nested lowering is complete, so final LIR removes it. The inner
    // primary pad remains, as do both handler cleanup pads.
    let pads: Vec<&str> = function
        .blocks
        .iter()
        .filter(|(_, block)| block.name.contains("try.unwind"))
        .map(|(_, block)| block.name.as_str())
        .collect();
    assert_eq!(pads.len(), 1);
    let cleanup_pad_count = function
        .blocks
        .iter()
        .filter(|(_, block)| {
            matches!(
                block.instructions.first(),
                Some(lir::Instruction::CleanupPad { .. })
            )
        })
        .count();
    assert_eq!(cleanup_pad_count, 2);
    let handler_pads: Vec<&str> = function
        .blocks
        .iter()
        .filter(|(_, block)| block.name.contains("handler_pad"))
        .map(|(_, block)| block.name.as_str())
        .collect();
    assert_eq!(handler_pads.len(), 2);
    // `a` unwinds to the inner catch pad. `b` runs inside that
    // handler and therefore unwinds through the inner cleanup;
    // `c` does the same through the outer cleanup.
    let mut invokes = Vec::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            if let lir::Instruction::Invoke { site } = instruction {
                invokes.push((
                    call_symbol(&module, site.destination(&function.call_targets)),
                    function.blocks[site.unwind()].name.as_str(),
                ));
            }
        }
    }
    let unwind_of = |symbol: &str| {
        invokes
            .iter()
            .find(|(s, _)| *s == symbol)
            .map(|(_, u)| *u)
            .unwrap_or_else(|| panic!("{symbol} must be invoked"))
    };
    assert_eq!(unwind_of("scoop.a"), pads[0]);
    assert_eq!(unwind_of("scoop.b"), handler_pads[1]);
    assert_eq!(unwind_of("scoop.c"), handler_pads[0]);
}
