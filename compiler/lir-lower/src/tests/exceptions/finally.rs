//! Finally execution on normal, catch, and early-return paths.

use super::*;

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
