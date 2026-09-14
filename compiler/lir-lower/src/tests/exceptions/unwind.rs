//! Throw, invoke-unwind, and nested exception-region lowering.

use super::*;

#[test]
fn throw_outside_try_is_a_throw_instruction() {
    // fun fail(): Unit { throw makeError() } — no try, so the
    // `Throw` instruction ends the block.
    let mut b = Builder::new();
    let my_error = my_error(&mut b);
    let mut ctor_locals = Arena::new();
    let make = b.user_fn_body(
        "makeError",
        Vec::new(),
        mir::Type::Class(my_error),
        returning_body(
            std::mem::take(&mut ctor_locals),
            expr(
                mir::Type::Class(my_error),
                mir::ExprKind::ClassAlloc { class_id: my_error },
            ),
        ),
    );
    let mut main_locals = Arena::new();
    let exception = main_locals.alloc(local("$call.1", mir::Type::Class(my_error)));
    let main = b.user_fn_body(
        "main",
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
                    pending: mir::CoroutinePendingContext::Root,
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
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e() -> ptr<managed>
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call managed-direct-target0 sp<managed-call:0> live=[] t0 = sig=direct0 (ptr<metadata>, machine<byte-size>) -> ptr<managed> runtime @scoop_rt_alloc(td0, machine<byte-size>(ByteSize(16)))
    ret t0
  fun @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee() -> void
    local %0 $call.1: ptr<managed>
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call managed-direct-target0 sp<managed-call:0> live=[] t0 = sig=direct0 () -> ptr<managed> local-fn0()
    store t0 -> local0
    throw local0
    unreachable
  td td0 MyError @scoop$1$td$eb205ad260a812589e9f030260657692c3e8a971a60e730337a3c28f28bc6cc9 type-id=1930812111026443540 shape=FixedObject minimum-size=16 align=8 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop$1$cb$92f24139c6f5bb3d64abf748dba9ff6099323c3e8df704588a886e027f85e4ee
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
    let main = b.user_fn_body("main", Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = &module.functions[module.entry.declaration().into_u32() as usize];
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
    let a = b.user_fn("a", Arena::new(), vec![]);
    let bb = b.user_fn("b", Arena::new(), vec![]);
    let c = b.user_fn("c", Arena::new(), vec![]);
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
    let main = b.user_fn_body("main", Vec::new(), mir::Type::Unit, body);
    let module = lower(&b.finish(main));

    let function = &module.functions[module.entry.declaration().into_u32() as usize];
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
    assert_eq!(unwind_of(module.functions[0].symbol()), pads[0]);
    assert_eq!(unwind_of(module.functions[1].symbol()), handler_pads[1]);
    assert_eq!(unwind_of(module.functions[2].symbol()), handler_pads[0]);
}
